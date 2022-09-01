use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Display;

use sctk::reexports::calloop;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use sctk::window::Decorations;

use crate::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOsError};
use crate::monitor::MonitorHandle as RootMonitorHandle;
use crate::platform_impl::{
    MonitorHandle as PlatformMonitorHandle, OsError,
    PlatformSpecificWindowBuilderAttributes as PlatformAttributes,
};
use crate::window::{
    CursorGrabMode, CursorIcon, Fullscreen, Theme, UserAttentionType, WindowAttributes,
};

use super::env::WindowingFeatures;
use super::event_loop::WinitState;
use super::output::{MonitorHandle, OutputManagerHandle};
use super::{EventLoopWindowTarget, WindowId};

pub mod shim;

use shim::{WindowCompositorUpdate, WindowHandle, WindowRequest, WindowUserRequest};

#[cfg(feature = "sctk-adwaita")]
pub type WinitFrame = sctk_adwaita::AdwaitaFrame;
#[cfg(not(feature = "sctk-adwaita"))]
pub type WinitFrame = sctk::window::FallbackFrame;

#[cfg(feature = "sctk-adwaita")]
const WAYLAND_CSD_THEME_ENV_VAR: &str = "WINIT_WAYLAND_CSD_THEME";

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

    /// Whether the window is resizeable.
    resizeable: AtomicBool,

    /// Whether the window is decorated.
    decorated: AtomicBool,

    /// Grabbing mode.
    cursor_grab_mode: Mutex<CursorGrabMode>,
}

impl Window {
    pub(crate) fn new<T>(
        event_loop_window_target: &EventLoopWindowTarget<T>,
        attributes: WindowAttributes,
        platform_attributes: PlatformAttributes,
    ) -> Result<Self, RootOsError> {
        let surface = event_loop_window_target
            .env
            .create_surface_with_scale_callback(move |scale, surface, mut dispatch_data| {
                let winit_state = dispatch_data.get::<WinitState>().unwrap();

                // Get the window that received the event.
                let window_id = super::make_wid(&surface);
                let mut window_compositor_update = winit_state
                    .window_compositor_updates
                    .get_mut(&window_id)
                    .unwrap();

                // Mark that we need a frame refresh on the DPI change.
                winit_state
                    .window_user_requests
                    .get_mut(&window_id)
                    .unwrap()
                    .refresh_frame = true;

                // Set pending scale factor.
                window_compositor_update.scale_factor = Some(scale);

                surface.set_buffer_scale(scale);
            })
            .detach();

        let scale_factor = sctk::get_surface_scale_factor(&surface);

        let window_id = super::make_wid(&surface);
        let maximized = Arc::new(AtomicBool::new(false));
        let maximized_clone = maximized.clone();
        let fullscreen = Arc::new(AtomicBool::new(false));
        let fullscreen_clone = fullscreen.clone();

        let (width, height) = attributes
            .inner_size
            .map(|size| size.to_logical::<f64>(scale_factor as f64).into())
            .unwrap_or((800, 600));

        let theme_manager = event_loop_window_target.theme_manager.clone();
        let mut window = event_loop_window_target
            .env
            .create_window::<WinitFrame, _>(
                surface.clone(),
                Some(theme_manager),
                (width, height),
                move |event, mut dispatch_data| {
                    use sctk::window::{Event, State};

                    let winit_state = dispatch_data.get::<WinitState>().unwrap();
                    let mut window_compositor_update = winit_state
                        .window_compositor_updates
                        .get_mut(&window_id)
                        .unwrap();

                    let mut window_user_requests = winit_state
                        .window_user_requests
                        .get_mut(&window_id)
                        .unwrap();

                    match event {
                        Event::Refresh => {
                            window_user_requests.refresh_frame = true;
                        }
                        Event::Configure { new_size, states } => {
                            let is_maximized = states.contains(&State::Maximized);
                            maximized_clone.store(is_maximized, Ordering::Relaxed);
                            let is_fullscreen = states.contains(&State::Fullscreen);
                            fullscreen_clone.store(is_fullscreen, Ordering::Relaxed);

                            window_user_requests.refresh_frame = true;
                            if let Some((w, h)) = new_size {
                                window_compositor_update.size = Some(LogicalSize::new(w, h));
                            }
                        }
                        Event::Close => {
                            window_compositor_update.close_window = true;
                        }
                    }
                },
            )
            .map_err(|_| os_error!(OsError::WaylandMisc("failed to create window.")))?;

        // Set CSD frame config from theme if specified,
        // otherwise use upstream automatic selection.
        #[cfg(feature = "sctk-adwaita")]
        if let Some(theme) = platform_attributes.csd_theme.or_else(|| {
            std::env::var(WAYLAND_CSD_THEME_ENV_VAR)
                .ok()
                .and_then(|s| s.as_str().try_into().ok())
        }) {
            window.set_frame_config(theme.into());
        }

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
        if let Some(name) = platform_attributes.name {
            window.set_app_id(name.general);
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

        // Without this commit here at least on kwin 5.23.3 the initial configure
        // will have a size (1,1), the second configure including the decoration
        // mode will have the min_size as its size. With this commit the initial
        // configure will have no size, the application will draw it's content
        // with the initial size and everything works as expected afterwards.
        //
        // The window commit must be after setting on top level properties, but right before any
        // buffer attachments commits.
        window.surface().commit();

        let size = Arc::new(Mutex::new(LogicalSize::new(width, height)));

        // We should trigger redraw and commit the surface for the newly created window.
        let mut window_user_request = WindowUserRequest::new();
        window_user_request.refresh_frame = true;
        window_user_request.redraw_requested = true;

        let window_id = super::make_wid(&surface);
        let window_requests = Arc::new(Mutex::new(Vec::with_capacity(64)));

        // Create a handle that performs all the requests on underlying sctk a window.
        let window_handle = WindowHandle::new(
            &event_loop_window_target.env,
            window,
            size.clone(),
            window_requests.clone(),
        );

        // Set resizable state, so we can determine how to handle `Window::set_inner_size`.
        window_handle.is_resizable.set(attributes.resizable);

        let mut winit_state = event_loop_window_target.state.borrow_mut();

        winit_state.window_map.insert(window_id, window_handle);

        // On Wayland window doesn't have Focus by default and it'll get it later on. So be
        // explicit here.
        winit_state
            .event_sink
            .push_window_event(crate::event::WindowEvent::Focused(false), window_id);

        // Add state for the window.
        winit_state
            .window_user_requests
            .insert(window_id, window_user_request);
        winit_state
            .window_compositor_updates
            .insert(window_id, WindowCompositorUpdate::new());

        let windowing_features = event_loop_window_target.windowing_features;

        // To make our window usable for drawing right away we must `ack` a `configure`
        // from the server, the acking part here is done by SCTK window frame, so we just
        // need to sync with server so it'll be done automatically for us.
        {
            let mut wayland_source = event_loop_window_target.wayland_dispatcher.as_source_mut();
            let event_queue = wayland_source.queue();
            let _ = event_queue.sync_roundtrip(&mut *winit_state, |_, _, _| unreachable!());
        }

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
            resizeable: AtomicBool::new(attributes.resizable),
            decorated: AtomicBool::new(attributes.decorations),
            cursor_grab_mode: Mutex::new(CursorGrabMode::None),
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
        self.send_request(WindowRequest::Title(title.to_owned()));
    }

    #[inline]
    pub fn set_visible(&self, _visible: bool) {
        // Not possible on Wayland.
    }

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        None
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
        self.send_request(WindowRequest::Redraw);
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

        self.send_request(WindowRequest::FrameSize(size));
    }

    #[inline]
    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        let scale_factor = self.scale_factor() as f64;
        let size = dimensions.map(|size| size.to_logical::<u32>(scale_factor));

        self.send_request(WindowRequest::MinSize(size));
    }

    #[inline]
    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        let scale_factor = self.scale_factor() as f64;
        let size = dimensions.map(|size| size.to_logical::<u32>(scale_factor));

        self.send_request(WindowRequest::MaxSize(size));
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        self.resizeable.store(resizable, Ordering::Relaxed);
        self.send_request(WindowRequest::Resizeable(resizable));
    }

    #[inline]
    pub fn is_resizable(&self) -> bool {
        self.resizeable.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn scale_factor(&self) -> u32 {
        // The scale factor from `get_surface_scale_factor` is always greater than zero, so
        // u32 conversion is safe.
        sctk::get_surface_scale_factor(&self.surface) as u32
    }

    #[inline]
    pub fn set_decorations(&self, decorate: bool) {
        self.decorated.store(decorate, Ordering::Relaxed);
        self.send_request(WindowRequest::Decorate(decorate));
    }

    #[inline]
    pub fn is_decorated(&self) -> bool {
        self.decorated.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn set_csd_theme(&self, theme: Theme) {
        self.send_request(WindowRequest::CsdThemeVariant(theme));
    }

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        // You can't unminimize the window on Wayland.
        if !minimized {
            return;
        }

        self.send_request(WindowRequest::Minimize);
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        self.maximized.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        self.send_request(WindowRequest::Maximize(maximized));
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

        self.send_request(fullscreen_request);
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        self.send_request(WindowRequest::NewCursorIcon(cursor));
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        self.send_request(WindowRequest::ShowCursor(visible));
    }

    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        if !self.windowing_features.pointer_constraints() {
            if mode == CursorGrabMode::None {
                return Ok(());
            }

            return Err(ExternalError::NotSupported(NotSupportedError::new()));
        }

        *self.cursor_grab_mode.lock().unwrap() = mode;
        self.send_request(WindowRequest::SetCursorGrabMode(mode));

        Ok(())
    }

    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        if !self.windowing_features.xdg_activation() {
            warn!("`request_user_attention` isn't supported");
            return;
        }

        self.send_request(WindowRequest::Attention(request_type));
    }

    #[inline]
    pub fn set_cursor_position(&self, position: Position) -> Result<(), ExternalError> {
        // Positon can be set only for locked cursor.
        if *self.cursor_grab_mode.lock().unwrap() != CursorGrabMode::Locked {
            return Err(ExternalError::Os(os_error!(OsError::WaylandMisc(
                "cursor position can be set only for locked cursor."
            ))));
        }

        let scale_factor = self.scale_factor() as f64;
        let position = position.to_logical(scale_factor);
        self.send_request(WindowRequest::SetLockedCursorPosition(position));

        Ok(())
    }

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        self.send_request(WindowRequest::DragWindow);

        Ok(())
    }

    #[inline]
    pub fn set_cursor_hittest(&self, hittest: bool) -> Result<(), ExternalError> {
        self.send_request(WindowRequest::PassthroughMouseInput(!hittest));

        Ok(())
    }

    #[inline]
    pub fn set_ime_position(&self, position: Position) {
        let scale_factor = self.scale_factor() as f64;
        let position = position.to_logical(scale_factor);
        self.send_request(WindowRequest::ImePosition(position));
    }

    #[inline]
    pub fn set_ime_allowed(&self, allowed: bool) {
        self.send_request(WindowRequest::AllowIme(allowed));
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
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let mut window_handle = WaylandWindowHandle::empty();
        window_handle.surface = self.surface.as_ref().c_ptr() as *mut _;
        RawWindowHandle::Wayland(window_handle)
    }

    #[inline]
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        let mut display_handle = WaylandDisplayHandle::empty();
        display_handle.display = self.display.get_display_ptr() as *mut _;
        RawDisplayHandle::Wayland(display_handle)
    }

    #[inline]
    fn send_request(&self, request: WindowRequest) {
        self.window_requests.lock().unwrap().push(request);
        self.event_loop_awakener.ping();
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.send_request(WindowRequest::Close);
    }
}

#[cfg(feature = "sctk-adwaita")]
impl From<Theme> for sctk_adwaita::FrameConfig {
    fn from(theme: Theme) -> Self {
        match theme {
            Theme::Light => sctk_adwaita::FrameConfig::light(),
            Theme::Dark => sctk_adwaita::FrameConfig::dark(),
        }
    }
}

impl TryFrom<&str> for Theme {
    type Error = ();

    /// ```
    /// use winit::window::Theme;
    ///
    /// assert_eq!("dark".try_into(), Ok(Theme::Dark));
    /// assert_eq!("lIghT".try_into(), Ok(Theme::Light));
    /// ```
    fn try_from(theme: &str) -> Result<Self, Self::Error> {
        if theme.eq_ignore_ascii_case("dark") {
            Ok(Self::Dark)
        } else if theme.eq_ignore_ascii_case("light") {
            Ok(Self::Light)
        } else {
            Err(())
        }
    }
}
