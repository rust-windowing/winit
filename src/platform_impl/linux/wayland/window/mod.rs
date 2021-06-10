use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Display;

use sctk::reexports::calloop;

use sctk::window::{
    ARGBColor, ButtonColorSpec, ColorSpec, ConceptConfig, ConceptFrame, Decorations,
};

use raw_window_handle::unix::WaylandHandle;

use crate::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOsError};
use crate::monitor::MonitorHandle as RootMonitorHandle;
use crate::platform::unix::{ARGBColor as LocalARGBColor, Button, ButtonState, Element, Theme};
use crate::platform_impl::{
    MonitorHandle as PlatformMonitorHandle, OsError,
    PlatformSpecificWindowBuilderAttributes as PlatformAttributes,
};
use crate::window::{CursorIcon, Fullscreen, WindowAttributes};

use super::env::WindowingFeatures;
use super::event_loop::WinitState;
use super::output::{MonitorHandle, OutputManagerHandle};
use super::{EventLoopWindowTarget, WindowId};

pub mod shim;

use shim::{WindowHandle, WindowRequest, WindowUpdate};

pub struct Window {
    /// Window id.
    window_id: WindowId,

    /// The Wayland display.
    display: Display,

    /// The underlying wl_surface.
    surface: WlSurface,

    /// The current window size.
    size: Arc<Mutex<LogicalSize<u32>>>,

    /// A handle to output manager.
    output_manager_handle: OutputManagerHandle,

    /// Event loop proxy to wake it up.
    event_loop_awakener: calloop::ping::Ping,

    /// Fullscreen state.
    fullscreen: Arc<AtomicBool>,

    /// Maximized state.
    maximized: Arc<AtomicBool>,

    /// Available windowing features.
    windowing_features: WindowingFeatures,

    /// Requests that SCTK window should perform.
    window_requests: Arc<Mutex<Vec<WindowRequest>>>,
}

impl Window {
    pub fn new<T>(
        event_loop_window_target: &EventLoopWindowTarget<T>,
        attributes: WindowAttributes,
        platform_attributes: PlatformAttributes,
    ) -> Result<Self, RootOsError> {
        let surface = event_loop_window_target
            .env
            .create_surface_with_scale_callback(move |scale, surface, mut dispatch_data| {
                let winit_state = dispatch_data.get::<WinitState>().unwrap();

                // Get the window that receiced the event.
                let window_id = super::make_wid(&surface);
                let mut window_update = winit_state.window_updates.get_mut(&window_id).unwrap();

                // Set pending scale factor.
                window_update.scale_factor = Some(scale);
                window_update.redraw_requested = true;

                surface.set_buffer_scale(scale);
            })
            .detach();

        let scale_factor = sctk::get_surface_scale_factor(&surface);

        let window_id = super::make_wid(&surface);
        let maximized = Arc::new(AtomicBool::new(false));
        let maximzied_clone = maximized.clone();
        let fullscreen = Arc::new(AtomicBool::new(false));
        let fullscreen_clone = fullscreen.clone();

        let (width, height) = attributes
            .inner_size
            .map(|size| size.to_logical::<f64>(scale_factor as f64).into())
            .unwrap_or((800, 600));

        let theme_manager = event_loop_window_target.theme_manager.clone();
        let mut window = event_loop_window_target
            .env
            .create_window::<ConceptFrame, _>(
                surface.clone(),
                Some(theme_manager),
                (width, height),
                move |event, mut dispatch_data| {
                    use sctk::window::{Event, State};

                    let winit_state = dispatch_data.get::<WinitState>().unwrap();
                    let mut window_update = winit_state.window_updates.get_mut(&window_id).unwrap();

                    match event {
                        Event::Refresh => {
                            window_update.refresh_frame = true;
                        }
                        Event::Configure { new_size, states } => {
                            let is_maximized = states.contains(&State::Maximized);
                            maximzied_clone.store(is_maximized, Ordering::Relaxed);
                            let is_fullscreen = states.contains(&State::Fullscreen);
                            fullscreen_clone.store(is_fullscreen, Ordering::Relaxed);

                            window_update.refresh_frame = true;
                            window_update.redraw_requested = true;
                            if let Some((w, h)) = new_size {
                                window_update.size = Some(LogicalSize::new(w, h));
                            }
                        }
                        Event::Close => {
                            window_update.close_window = true;
                        }
                    }
                },
            )
            .map_err(|_| os_error!(OsError::WaylandMisc("failed to create window.")))?;

        // Set decorations.
        if attributes.decorations {
            window.set_decorate(Decorations::FollowServer);
        } else {
            window.set_decorate(Decorations::None);
        }

        // Min dimensions.
        let min_size = attributes
            .min_inner_size
            .map(|size| size.to_logical::<f64>(scale_factor as f64).into());
        window.set_min_size(min_size);

        // Max dimensions.
        let max_size = attributes
            .max_inner_size
            .map(|size| size.to_logical::<f64>(scale_factor as f64).into());
        window.set_max_size(max_size);

        // Set Wayland specific window attributes.
        if let Some(app_id) = platform_attributes.app_id {
            window.set_app_id(app_id);
        }

        // Set common window attributes.
        //
        // We set resizable after other attributes, since it touches min and max size under
        // the hood.
        window.set_resizable(attributes.resizable);
        window.set_title(attributes.title);

        // Set fullscreen/maximized if so was requested.
        match attributes.fullscreen {
            Some(Fullscreen::Exclusive(_)) => {
                warn!("`Fullscreen::Exclusive` is ignored on Wayland")
            }
            Some(Fullscreen::Borderless(monitor)) => {
                let monitor =
                    monitor.and_then(|RootMonitorHandle { inner: monitor }| match monitor {
                        PlatformMonitorHandle::Wayland(monitor) => Some(monitor.proxy),
                        #[cfg(feature = "x11")]
                        PlatformMonitorHandle::X(_) => None,
                    });

                window.set_fullscreen(monitor.as_ref());
            }
            None => {
                if attributes.maximized {
                    window.set_maximized();
                }
            }
        }

        let size = Arc::new(Mutex::new(LogicalSize::new(width, height)));

        // We should trigger redraw and commit the surface for the newly created window.
        let mut window_update = WindowUpdate::new();
        window_update.refresh_frame = true;
        window_update.redraw_requested = true;

        let window_id = super::make_wid(&surface);
        let window_requests = Arc::new(Mutex::new(Vec::with_capacity(64)));

        // Create a handle that performs all the requests on underlying sctk a window.
        let window_handle = WindowHandle::new(window, size.clone(), window_requests.clone());

        let mut winit_state = event_loop_window_target.state.borrow_mut();

        winit_state.window_map.insert(window_id, window_handle);

        winit_state
            .window_updates
            .insert(window_id, WindowUpdate::new());

        let windowing_features = event_loop_window_target.windowing_features;

        // Send all updates to the server.
        let wayland_source = &event_loop_window_target.wayland_source;
        let event_loop_handle = &event_loop_window_target.event_loop_handle;

        // To make our window usable for drawing right away we must `ack` a `configure`
        // from the server, the acking part here is done by SCTK window frame, so we just
        // need to sync with server so it'll be done automatically for us.
        event_loop_handle.with_source(&wayland_source, |event_queue| {
            let event_queue = event_queue.queue();
            let _ = event_queue.sync_roundtrip(&mut *winit_state, |_, _, _| unreachable!());
        });

        // We all praise GNOME for these 3 lines of pure magic. If we don't do that,
        // GNOME will shrink our window a bit for the size of the decorations. I guess it
        // happens because we haven't committed them with buffers to the server.
        let window_handle = winit_state.window_map.get_mut(&window_id).unwrap();
        window_handle.window.refresh();

        let output_manager_handle = event_loop_window_target.output_manager.handle();

        let window = Self {
            window_id,
            surface,
            display: event_loop_window_target.display.clone(),
            output_manager_handle,
            size,
            window_requests,
            event_loop_awakener: event_loop_window_target.event_loop_awakener.clone(),
            fullscreen,
            maximized,
            windowing_features,
        };

        Ok(window)
    }
}

impl Window {
    #[inline]
    pub fn id(&self) -> WindowId {
        self.window_id
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        let title_request = WindowRequest::Title(title.to_owned());
        self.window_requests.lock().unwrap().push(title_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn set_visible(&self, _visible: bool) {
        // Not possible on Wayland.
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn set_outer_position(&self, _: Position) {
        // Not possible on Wayland.
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.size
            .lock()
            .unwrap()
            .to_physical(self.scale_factor() as f64)
    }

    #[inline]
    pub fn request_redraw(&self) {
        let redraw_request = WindowRequest::Redraw;
        self.window_requests.lock().unwrap().push(redraw_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        self.size
            .lock()
            .unwrap()
            .to_physical(self.scale_factor() as f64)
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        let scale_factor = self.scale_factor() as f64;

        let size = size.to_logical::<u32>(scale_factor);
        *self.size.lock().unwrap() = size;

        let frame_size_request = WindowRequest::FrameSize(size);
        self.window_requests
            .lock()
            .unwrap()
            .push(frame_size_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        let scale_factor = self.scale_factor() as f64;
        let size = dimensions.map(|size| size.to_logical::<u32>(scale_factor));

        let min_size_request = WindowRequest::MinSize(size);
        self.window_requests.lock().unwrap().push(min_size_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        let scale_factor = self.scale_factor() as f64;
        let size = dimensions.map(|size| size.to_logical::<u32>(scale_factor));

        let max_size_request = WindowRequest::MaxSize(size);
        self.window_requests.lock().unwrap().push(max_size_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        let resizeable_request = WindowRequest::Resizeable(resizable);
        self.window_requests
            .lock()
            .unwrap()
            .push(resizeable_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn scale_factor(&self) -> u32 {
        // The scale factor from `get_surface_scale_factor` is always greater than zero, so
        // u32 conversion is safe.
        sctk::get_surface_scale_factor(&self.surface) as u32
    }

    #[inline]
    pub fn set_decorations(&self, decorate: bool) {
        let decorate_request = WindowRequest::Decorate(decorate);
        self.window_requests.lock().unwrap().push(decorate_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        // You can't unminimize the window on Wayland.
        if !minimized {
            return;
        }

        let minimize_request = WindowRequest::Minimize;
        self.window_requests.lock().unwrap().push(minimize_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        self.maximized.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        let maximize_request = WindowRequest::Maximize(maximized);
        self.window_requests.lock().unwrap().push(maximize_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        if self.fullscreen.load(Ordering::Relaxed) {
            let current_monitor = self.current_monitor().map(|monitor| RootMonitorHandle {
                inner: PlatformMonitorHandle::Wayland(monitor),
            });

            Some(Fullscreen::Borderless(current_monitor))
        } else {
            None
        }
    }

    #[inline]
    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        let fullscreen_request = match fullscreen {
            Some(Fullscreen::Exclusive(_)) => {
                warn!("`Fullscreen::Exclusive` is ignored on Wayland");
                return;
            }
            Some(Fullscreen::Borderless(monitor)) => {
                let monitor =
                    monitor.and_then(|RootMonitorHandle { inner: monitor }| match monitor {
                        PlatformMonitorHandle::Wayland(monitor) => Some(monitor.proxy),
                        #[cfg(feature = "x11")]
                        PlatformMonitorHandle::X(_) => None,
                    });

                WindowRequest::Fullscreen(monitor)
            }
            None => WindowRequest::UnsetFullscreen,
        };

        self.window_requests
            .lock()
            .unwrap()
            .push(fullscreen_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn set_theme<T: Theme>(&self, theme: T) {
        // First buttons is minimize, then maximize, and then close.
        let buttons: Vec<(ButtonColorSpec, ButtonColorSpec)> =
            [Button::Minimize, Button::Maximize, Button::Close]
                .iter()
                .map(|button| {
                    let button = *button;
                    let idle_active_bg = theme
                        .button_color(button, ButtonState::Idle, false, true)
                        .into();
                    let idle_inactive_bg = theme
                        .button_color(button, ButtonState::Idle, false, false)
                        .into();
                    let idle_active_icon = theme
                        .button_color(button, ButtonState::Idle, true, true)
                        .into();
                    let idle_inactive_icon = theme
                        .button_color(button, ButtonState::Idle, true, false)
                        .into();
                    let idle_bg = ColorSpec {
                        active: idle_active_bg,
                        inactive: idle_inactive_bg,
                    };
                    let idle_icon = ColorSpec {
                        active: idle_active_icon,
                        inactive: idle_inactive_icon,
                    };

                    let hovered_active_bg = theme
                        .button_color(button, ButtonState::Hovered, false, true)
                        .into();
                    let hovered_inactive_bg = theme
                        .button_color(button, ButtonState::Hovered, false, false)
                        .into();
                    let hovered_active_icon = theme
                        .button_color(button, ButtonState::Hovered, true, true)
                        .into();
                    let hovered_inactive_icon = theme
                        .button_color(button, ButtonState::Hovered, true, false)
                        .into();
                    let hovered_bg = ColorSpec {
                        active: hovered_active_bg,
                        inactive: hovered_inactive_bg,
                    };
                    let hovered_icon = ColorSpec {
                        active: hovered_active_icon,
                        inactive: hovered_inactive_icon,
                    };

                    let disabled_active_bg = theme
                        .button_color(button, ButtonState::Disabled, false, true)
                        .into();
                    let disabled_inactive_bg = theme
                        .button_color(button, ButtonState::Disabled, false, false)
                        .into();
                    let disabled_active_icon = theme
                        .button_color(button, ButtonState::Disabled, true, true)
                        .into();
                    let disabled_inactive_icon = theme
                        .button_color(button, ButtonState::Disabled, true, false)
                        .into();
                    let disabled_bg = ColorSpec {
                        active: disabled_active_bg,
                        inactive: disabled_inactive_bg,
                    };
                    let disabled_icon = ColorSpec {
                        active: disabled_active_icon,
                        inactive: disabled_inactive_icon,
                    };

                    let button_bg = ButtonColorSpec {
                        idle: idle_bg,
                        hovered: hovered_bg,
                        disabled: disabled_bg,
                    };
                    let button_icon = ButtonColorSpec {
                        idle: idle_icon,
                        hovered: hovered_icon,
                        disabled: disabled_icon,
                    };

                    (button_icon, button_bg)
                })
                .collect();

        let minimize_button = Some(buttons[0]);
        let maximize_button = Some(buttons[1]);
        let close_button = Some(buttons[2]);

        // The first color is bar, then separator, and then text color.
        let titlebar_colors: Vec<ColorSpec> = [Element::Bar, Element::Separator, Element::Text]
            .iter()
            .map(|element| {
                let element = *element;
                let active = theme.element_color(element, true).into();
                let inactive = theme.element_color(element, false).into();

                ColorSpec { active, inactive }
            })
            .collect();

        let primary_color = titlebar_colors[0];
        let secondary_color = titlebar_colors[1];
        let title_color = titlebar_colors[2];

        let title_font = theme.font();

        let concept_config = ConceptConfig {
            primary_color,
            secondary_color,
            title_color,
            title_font,
            minimize_button,
            maximize_button,
            close_button,
        };

        let theme_request = WindowRequest::Theme(concept_config);
        self.window_requests.lock().unwrap().push(theme_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        let cursor_icon_request = WindowRequest::NewCursorIcon(cursor);
        self.window_requests
            .lock()
            .unwrap()
            .push(cursor_icon_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        let cursor_visible_request = WindowRequest::ShowCursor(visible);
        self.window_requests
            .lock()
            .unwrap()
            .push(cursor_visible_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn set_cursor_grab(&self, grab: bool) -> Result<(), ExternalError> {
        if !self.windowing_features.cursor_grab() {
            return Err(ExternalError::NotSupported(NotSupportedError::new()));
        }

        let cursor_grab_request = WindowRequest::GrabCursor(grab);
        self.window_requests
            .lock()
            .unwrap()
            .push(cursor_grab_request);
        self.event_loop_awakener.ping();

        Ok(())
    }

    #[inline]
    pub fn set_cursor_position(&self, _: Position) -> Result<(), ExternalError> {
        // XXX This is possible if the locked pointer is being used. We don't have any
        // API for that right now, but it could be added in
        // https://github.com/rust-windowing/winit/issues/1677.
        //
        // This function is essential for the locked pointer API.
        //
        // See pointer-constraints-unstable-v1.xml.
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        let drag_window_request = WindowRequest::DragWindow;
        self.window_requests
            .lock()
            .unwrap()
            .push(drag_window_request);
        self.event_loop_awakener.ping();

        Ok(())
    }

    #[inline]
    pub fn set_ime_position(&self, position: Position) {
        let scale_factor = self.scale_factor() as f64;
        let position = position.to_logical(scale_factor);
        let ime_position_request = WindowRequest::IMEPosition(position);
        self.window_requests
            .lock()
            .unwrap()
            .push(ime_position_request);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn display(&self) -> &Display {
        &self.display
    }

    #[inline]
    pub fn surface(&self) -> &WlSurface {
        &self.surface
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        let output = sctk::get_surface_outputs(&self.surface).last()?.clone();
        Some(MonitorHandle::new(output))
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        self.output_manager_handle.available_outputs()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        None
    }

    #[inline]
    pub fn raw_window_handle(&self) -> WaylandHandle {
        let display = self.display.get_display_ptr() as *mut _;
        let surface = self.surface.as_ref().c_ptr() as *mut _;

        WaylandHandle {
            display,
            surface,
            ..WaylandHandle::empty()
        }
    }
}

impl From<LocalARGBColor> for ARGBColor {
    fn from(color: LocalARGBColor) -> Self {
        let a = color.a;
        let r = color.r;
        let g = color.g;
        let b = color.b;
        Self { a, r, g, b }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        let close_request = WindowRequest::Close;
        self.window_requests.lock().unwrap().push(close_request);
        self.event_loop_awakener.ping();
    }
}
