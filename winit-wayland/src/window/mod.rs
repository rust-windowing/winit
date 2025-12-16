//! The Wayland window.

use std::ffi::c_void;
use std::fmt::Debug;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use dpi::{
    LogicalPosition, LogicalSize, PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size,
};
use sctk::compositor::{CompositorState, Region, SurfaceData};
use sctk::reexports::client::protocol::wl_display::WlDisplay;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{Proxy, QueueHandle};
use sctk::reexports::protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;
use sctk::shell::xdg::popup::Popup;
use sctk::shell::xdg::window::{Window as SctkWindow, WindowConfigure, WindowDecorations};
use sctk::shell::xdg::XdgPositioner;
use sctk::shell::{xdg, WaylandSurface};
use state::AnyWindowState;
use tracing::warn;
use wayland_protocols::xdg::shell::client::xdg_positioner::{
    Anchor, ConstraintAdjustment, Gravity,
};
use wayland_protocols::xdg::shell::client::xdg_surface;
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event::{Ime, WindowEvent};
use winit_core::event_loop::AsyncRequestSerial;
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::popup::{Direction, PopupAttributes};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindow, WindowAttributes, WindowButtons, WindowId,
    WindowLevel,
};

use super::ActiveEventLoop;
use super::event_loop::sink::EventSink;
use super::output::MonitorHandle;
use super::state::WinitState;
use super::types::xdg_activation::XdgActivationTokenData;
use crate::{WindowAttributesWayland, output};

pub(crate) mod state;

pub use state::WindowState;

pub trait WindowType: Sized {
    type Configure: Send + Sync + Debug;

    fn wl_surface(&self) -> &WlSurface;
    fn xdg_surface(&self) -> &xdg_surface::XdgSurface;

    // The rest of these functions are those that don't exist for popups
    fn set_min_surface_size(_: &Window<Self>, _: Option<Size>) {}
    fn set_max_surface_size(_: &Window<Self>, _: Option<Size>) {}
    fn set_title(_: &Mutex<WindowState<Self>>, _: &str) {}
    fn set_resizable(_: &Window<Self>, _: bool) {}
    fn set_minimized(&self, _: bool) {}
    fn set_maximized(&self, _: bool) {}
    fn is_maximized(_: &Mutex<WindowState<Self>>) -> bool {
        false
    }
    fn set_fullscreen(&self, _: Option<Fullscreen>) {}
    fn fullscreen(_: &Window<Self>) -> Option<Fullscreen> {
        None
    }
    fn set_decorations(_: &Mutex<WindowState<Self>>, _: bool) {}
    fn is_decorated(_: &Mutex<WindowState<Self>>) -> bool {
        false
    }
    fn set_window_icon(_: &Mutex<WindowState<Self>>, _: Option<winit_core::icon::Icon>) {}
    fn drag_window(_: &Mutex<WindowState<Self>>) -> Result<(), RequestError> {
        Err(NotSupportedError::new("popups can't be dragged on wayland").into())
    }
    fn drag_resize_window(
        _: &Mutex<WindowState<Self>>,
        _: ResizeDirection,
    ) -> Result<(), RequestError> {
        Err(NotSupportedError::new("popups can't be drag resized on wayland").into())
    }
    fn show_window_menu(_: &Mutex<WindowState<Self>>, _: Position) {}

    fn is_stateless(_: &Self::Configure) -> bool {
        true
    }
}

impl WindowType for SctkWindow {
    type Configure = WindowConfigure;

    fn wl_surface(&self) -> &WlSurface {
        WaylandSurface::wl_surface(self)
    }

    fn xdg_surface(&self) -> &xdg_surface::XdgSurface {
        xdg::XdgSurface::xdg_surface(self)
    }

    fn set_min_surface_size(window: &Window<SctkWindow>, min_size: Option<Size>) {
        let mut window_state = window.window_state.lock().unwrap();

        let size = min_size.map(|size| size.to_logical(window_state.scale_factor()));
        window_state.set_min_surface_size(size);

        // NOTE: Requires commit to be applied.
        window.request_redraw();
    }

    fn set_max_surface_size(window: &Window<SctkWindow>, max_size: Option<Size>) {
        let mut window_state = window.window_state.lock().unwrap();

        let size = max_size.map(|size| size.to_logical(window_state.scale_factor()));
        window_state.set_max_surface_size(size);

        // NOTE: Requires commit to be applied.
        window.request_redraw();
    }

    fn set_title(window_state: &Mutex<WindowState<SctkWindow>>, title: &str) {
        let new_title = title.to_string();
        window_state.lock().unwrap().set_title(new_title);
    }

    fn set_resizable(window: &Window<SctkWindow>, resizable: bool) {
        if window.window_state.lock().unwrap().set_resizable(resizable) {
            // NOTE: Requires commit to be applied.
            window.request_redraw();
        }
    }

    fn set_minimized(&self, minimized: bool) {
        // You can't unminimize the window on Wayland.
        if !minimized {
            warn!("Unminimizing is ignored on Wayland.");
            return;
        }

        self.set_minimized();
    }

    fn set_maximized(&self, maximized: bool) {
        if maximized {
            self.set_maximized()
        } else {
            self.unset_maximized()
        }
    }

    fn is_maximized(window_state: &Mutex<WindowState<SctkWindow>>) -> bool {
        window_state
            .lock()
            .unwrap()
            .last_configure
            .as_ref()
            .map(|last_configure| last_configure.is_maximized())
            .unwrap_or_default()
    }

    fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        match fullscreen {
            Some(Fullscreen::Exclusive(..)) => {
                warn!("`Fullscreen::Exclusive` is ignored on Wayland");
            },
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.as_ref().and_then(|monitor| {
                    monitor.cast_ref::<output::MonitorHandle>().map(|handle| &handle.proxy)
                });

                self.set_fullscreen(output)
            },
            None => self.unset_fullscreen(),
        }
    }

    fn fullscreen(window: &Window<SctkWindow>) -> Option<Fullscreen> {
        let is_fullscreen = window
            .window_state
            .lock()
            .unwrap()
            .last_configure
            .as_ref()
            .map(|last_configure| last_configure.is_fullscreen())
            .unwrap_or_default();

        if is_fullscreen {
            let current_monitor = window.current_monitor();
            Some(Fullscreen::Borderless(current_monitor))
        } else {
            None
        }
    }

    fn set_decorations(window_state: &Mutex<WindowState<SctkWindow>>, decorate: bool) {
        window_state.lock().unwrap().set_decorate(decorate)
    }

    fn is_decorated(window_state: &Mutex<WindowState<SctkWindow>>) -> bool {
        window_state.lock().unwrap().is_decorated()
    }

    fn set_window_icon(
        window_state: &Mutex<WindowState<SctkWindow>>,
        window_icon: Option<winit_core::icon::Icon>,
    ) {
        window_state.lock().unwrap().set_window_icon(window_icon);
    }

    fn drag_window(window_state: &Mutex<WindowState<SctkWindow>>) -> Result<(), RequestError> {
        window_state.lock().unwrap().drag_window()
    }

    fn drag_resize_window(
        window_state: &Mutex<WindowState<SctkWindow>>,
        direction: ResizeDirection,
    ) -> Result<(), RequestError> {
        window_state.lock().unwrap().drag_resize_window(direction)
    }

    fn show_window_menu(window_state: &Mutex<WindowState<SctkWindow>>, position: Position) {
        let window_state = window_state.lock().unwrap();

        window_state.show_window_menu(position.to_logical(window_state.scale_factor()));
    }

    fn is_stateless(configure: &WindowConfigure) -> bool {
        WindowState::<SctkWindow>::is_stateless(configure)
    }
}

impl WindowType for Popup {
    type Configure = ();

    fn wl_surface(&self) -> &WlSurface {
        self.wl_surface()
    }

    fn xdg_surface(&self) -> &xdg_surface::XdgSurface {
        self.xdg_surface()
    }
}

/// The Wayland window.
#[derive(Debug)]
pub struct Window<T: WindowType> {
    /// Reference to the underlying SCTK window/popup.
    window: T,

    /// Window id.
    window_id: WindowId,

    /// The state of the window.
    window_state: Arc<Mutex<WindowState<T>>>,

    /// Compositor to handle WlRegion stuff.
    compositor: Arc<CompositorState>,

    /// The wayland display used solely for raw window handle.
    #[allow(dead_code)]
    display: WlDisplay,

    /// Xdg activation to request user attention.
    xdg_activation: Option<XdgActivationV1>,

    /// The state of the requested attention from the `xdg_activation`.
    attention_requested: Arc<AtomicBool>,

    /// Handle to the main queue to perform requests.
    queue_handle: QueueHandle<WinitState>,

    /// Window requests to the event loop.
    window_requests: Arc<WindowRequests>,

    /// Observed monitors.
    monitors: Arc<Mutex<Vec<MonitorHandle>>>,

    /// Source to wake-up the event-loop for window requests.
    event_loop_awakener: calloop::ping::Ping,

    /// The event sink to deliver synthetic events.
    window_events_sink: Arc<Mutex<EventSink>>,
}

impl Window<SctkWindow> {
    pub(crate) fn new(
        event_loop_window_target: &ActiveEventLoop,
        mut attributes: WindowAttributes,
    ) -> Result<Self, RequestError> {
        let queue_handle = event_loop_window_target.queue_handle.clone();
        let mut state = event_loop_window_target.state.borrow_mut();

        let monitors = state.monitors.clone();

        let surface = state.compositor_state.create_surface(&queue_handle);
        let compositor = state.compositor_state.clone();
        let xdg_activation =
            state.xdg_activation.as_ref().map(|activation_state| activation_state.global().clone());
        let display = event_loop_window_target.handle.connection.display();

        let size: Size = attributes.surface_size.unwrap_or(LogicalSize::new(800., 600.).into());

        // We prefer server side decorations, however to not have decorations we ask for client
        // side decorations instead.
        let default_decorations = if attributes.decorations {
            WindowDecorations::RequestServer
        } else {
            WindowDecorations::RequestClient
        };

        let window =
            state.xdg_shell.create_window(surface.clone(), default_decorations, &queue_handle);

        let WindowAttributesWayland { name: app_name, activation_token, prefer_csd } = *attributes
            .platform
            .take()
            .and_then(|p| p.cast::<WindowAttributesWayland>().ok())
            .unwrap_or_default();

        let mut window_state = WindowState::new(
            event_loop_window_target.handle.clone(),
            &event_loop_window_target.queue_handle,
            &state,
            size,
            window.clone(),
            attributes.preferred_theme,
            prefer_csd,
        );

        window_state.set_window_icon(attributes.window_icon);

        // Set transparency hint.
        window_state.set_transparent(attributes.transparent);

        window_state.set_blur(attributes.blur);

        // Set the decorations hint.
        window_state.set_decorate(attributes.decorations);

        // Set the app_id.
        if let Some(name) = app_name.map(|name| name.general) {
            window.set_app_id(name);
        }

        // Set the window title.
        window_state.set_title(attributes.title);

        // Set the min and max sizes. We must set the hints upon creating a window, so
        // we use the default `1.` scaling...
        let min_size = attributes.min_surface_size.map(|size| size.to_logical(1.));
        let max_size = attributes.max_surface_size.map(|size| size.to_logical(1.));
        window_state.set_min_surface_size(min_size);
        window_state.set_max_surface_size(max_size);

        // Non-resizable implies that the min and max sizes are set to the same value.
        window_state.set_resizable(attributes.resizable);

        // Set startup mode.
        match attributes.fullscreen {
            Some(Fullscreen::Exclusive(..)) => {
                warn!("`Fullscreen::Exclusive` is ignored on Wayland");
            },
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.as_ref().and_then(|monitor| {
                    monitor.cast_ref::<output::MonitorHandle>().map(|handle| &handle.proxy)
                });

                window.set_fullscreen(output)
            },
            _ if attributes.maximized => window.set_maximized(),
            _ => (),
        };

        match attributes.cursor {
            Cursor::Icon(icon) => window_state.set_cursor(icon),
            Cursor::Custom(cursor) => window_state.set_custom_cursor(cursor),
        }

        // Activate the window when the token is passed.
        if let (Some(xdg_activation), Some(token)) = (xdg_activation.as_ref(), activation_token) {
            xdg_activation.activate(token.into_raw(), &surface);
        }

        // XXX Do initial commit.
        window.commit();

        // Add the window and window requests into the state.
        let window_state = Arc::new(Mutex::new(window_state));
        let window_id = super::make_wid(&surface);
        state.windows.get_mut().insert(window_id, AnyWindowState::TopLevel(window_state.clone()));

        let window_requests = WindowRequests {
            redraw_requested: AtomicBool::new(true),
            closed: AtomicBool::new(false),
        };
        let window_requests = Arc::new(window_requests);
        state.window_requests.get_mut().insert(window_id, window_requests.clone());

        // Setup the event sync to insert `WindowEvents` right from the window.
        let window_events_sink = state.window_events_sink.clone();

        let mut wayland_source = event_loop_window_target.wayland_dispatcher.as_source_mut();
        let event_queue = wayland_source.queue();

        // Do a roundtrip.
        event_queue.roundtrip(&mut state).map_err(|err| os_error!(err))?;

        // XXX Wait for the initial configure to arrive.
        while !window_state.lock().unwrap().is_configured() {
            event_queue.blocking_dispatch(&mut state).map_err(|err| os_error!(err))?;
        }

        // Wake-up event loop, so it'll send initial redraw requested.
        let event_loop_awakener = event_loop_window_target.event_loop_awakener.clone();
        event_loop_awakener.ping();

        Ok(Self {
            window,
            display,
            monitors,
            window_id,
            compositor,
            window_state,
            queue_handle,
            xdg_activation,
            attention_requested: Arc::new(AtomicBool::new(false)),
            event_loop_awakener,
            window_requests,
            window_events_sink,
        })
    }

    pub(crate) fn xdg_toplevel(&self) -> Option<NonNull<c_void>> {
        NonNull::new(self.window.xdg_toplevel().id().as_ptr().cast())
    }
}

impl Window<Popup> {
    pub(crate) fn new_popup(
        event_loop_window_target: &ActiveEventLoop,
        mut attributes: WindowAttributes,
        parent_id: WindowId,
        popup_attributes: PopupAttributes,
    ) -> Result<Self, RequestError> {
        let mut state = event_loop_window_target.state.borrow_mut();

        let Some(parent) = state.windows.get_mut().get(&parent_id) else {
            return Err(
                NotSupportedError::new("can't create a popup with a nonexistent parent").into()
            );
        };

        let parent = match parent {
            AnyWindowState::TopLevel(window) => window.lock().unwrap().window.xdg_surface().clone(),
            AnyWindowState::Popup(window) => window.lock().unwrap().window.xdg_surface().clone(),
        };

        let queue_handle = event_loop_window_target.queue_handle.clone();

        let monitors = state.monitors.clone();

        let surface = state.compositor_state.create_surface(&queue_handle);
        let compositor = state.compositor_state.clone();
        let xdg_activation =
            state.xdg_activation.as_ref().map(|activation_state| activation_state.global().clone());
        let display = event_loop_window_target.handle.connection.display();

        let size: Size = attributes.surface_size.unwrap_or(LogicalSize::new(800., 600.).into());

        // Create the positioner
        let positioner = XdgPositioner::new(&state.xdg_shell).unwrap();
        let logical_size = size.to_logical(1.);
        positioner.set_size(logical_size.width, logical_size.height);

        let position = attributes
            .position
            .map_or(LogicalPosition::default(), |position| position.to_logical(1.));
        let anchor_size = popup_attributes.anchor_size.to_logical::<i32>(1.);
        positioner.set_anchor_rect(
            position.x,
            position.y,
            anchor_size.width.max(1),
            anchor_size.height.max(1),
        );

        // TODO(bolshoytoster): is there a better way to convert between two identical enums?
        match popup_attributes.anchor {
            Direction::None => (),
            Direction::Top => positioner.set_anchor(Anchor::Top),
            Direction::Bottom => positioner.set_anchor(Anchor::Bottom),
            Direction::Left => positioner.set_anchor(Anchor::Left),
            Direction::Right => positioner.set_anchor(Anchor::Right),
            Direction::TopLeft => positioner.set_anchor(Anchor::TopLeft),
            Direction::BottomLeft => positioner.set_anchor(Anchor::BottomLeft),
            Direction::TopRight => positioner.set_anchor(Anchor::TopRight),
            Direction::BottomRight => positioner.set_anchor(Anchor::BottomRight),
        };
        match popup_attributes.gravity {
            Direction::None => (),
            Direction::Top => positioner.set_gravity(Gravity::Top),
            Direction::Bottom => positioner.set_gravity(Gravity::Bottom),
            Direction::Left => positioner.set_gravity(Gravity::Left),
            Direction::Right => positioner.set_gravity(Gravity::Right),
            Direction::TopLeft => positioner.set_gravity(Gravity::TopLeft),
            Direction::BottomLeft => positioner.set_gravity(Gravity::BottomLeft),
            Direction::TopRight => positioner.set_gravity(Gravity::TopRight),
            Direction::BottomRight => positioner.set_gravity(Gravity::BottomRight),
        };

        if !popup_attributes.anchor_hints.is_empty() {
            // The AnchorHints bitfield is identical to ConstraintAdjustment, so we can convert
            // between them safely
            positioner.set_constraint_adjustment(ConstraintAdjustment::from_bits_retain(
                popup_attributes.anchor_hints.bits() as u32,
            ));
        }

        let offset = popup_attributes.offset.to_logical(1.);
        if offset.x != 0 || offset.y != 0 {
            positioner.set_offset(offset.x, offset.y);
        }

        // Create the popup
        let window = Popup::from_surface(
            Some(&parent),
            &positioner,
            &queue_handle,
            surface.clone(),
            &state.xdg_shell,
        )
        .unwrap();

        let WindowAttributesWayland { activation_token, prefer_csd, .. } = *attributes
            .platform
            .take()
            .and_then(|p| p.cast::<WindowAttributesWayland>().ok())
            .unwrap_or_default();

        let mut window_state = WindowState::new(
            event_loop_window_target.handle.clone(),
            &event_loop_window_target.queue_handle,
            &state,
            size,
            window.clone(),
            attributes.preferred_theme,
            prefer_csd,
        );

        // Set transparency hint.
        window_state.set_transparent(attributes.transparent);

        window_state.set_blur(attributes.blur);

        match attributes.cursor {
            Cursor::Icon(icon) => window_state.set_cursor(icon),
            Cursor::Custom(cursor) => window_state.set_custom_cursor(cursor),
        }

        // Activate the window when the token is passed.
        if let (Some(xdg_activation), Some(token)) = (xdg_activation.as_ref(), activation_token) {
            xdg_activation.activate(token.into_raw(), &surface);
        }

        // XXX Do initial commit.
        window.wl_surface().commit();

        // Add the window and window requests into the state.
        let window_state = Arc::new(Mutex::new(window_state));
        let window_id = super::make_wid(&surface);
        state.windows.get_mut().insert(window_id, AnyWindowState::Popup(window_state.clone()));

        let window_requests = WindowRequests {
            redraw_requested: AtomicBool::new(true),
            closed: AtomicBool::new(false),
        };
        let window_requests = Arc::new(window_requests);
        state.window_requests.get_mut().insert(window_id, window_requests.clone());

        // Setup the event sync to insert `WindowEvents` right from the window.
        let window_events_sink = state.window_events_sink.clone();

        let mut wayland_source = event_loop_window_target.wayland_dispatcher.as_source_mut();
        let event_queue = wayland_source.queue();

        // Do a roundtrip.
        event_queue.roundtrip(&mut state).map_err(|err| os_error!(err))?;

        // XXX Wait for the initial configure to arrive.
        while !window_state.lock().unwrap().is_configured() {
            event_queue.blocking_dispatch(&mut state).map_err(|err| os_error!(err))?;
        }

        // Wake-up event loop, so it'll send initial redraw requested.
        let event_loop_awakener = event_loop_window_target.event_loop_awakener.clone();
        event_loop_awakener.ping();

        Ok(Self {
            window,
            display,
            monitors,
            window_id,
            compositor,
            window_state,
            queue_handle,
            xdg_activation,
            attention_requested: Arc::new(AtomicBool::new(false)),
            event_loop_awakener,
            window_requests,
            window_events_sink,
        })
    }
}

impl<T: WindowType> Window<T> {
    pub fn request_activation_token(&self) -> Result<AsyncRequestSerial, RequestError> {
        let xdg_activation = match self.xdg_activation.as_ref() {
            Some(xdg_activation) => xdg_activation,
            None => return Err(NotSupportedError::new("xdg_activation_v1 is not available").into()),
        };

        let serial = AsyncRequestSerial::get();

        let data = XdgActivationTokenData::Obtain((self.window_id, serial));
        let xdg_activation_token = xdg_activation.get_activation_token(&self.queue_handle, data);
        xdg_activation_token.set_surface(self.surface());
        xdg_activation_token.commit();

        Ok(serial)
    }

    #[inline]
    pub fn surface(&self) -> &WlSurface {
        self.window.wl_surface()
    }
}

impl<T: WindowType> Drop for Window<T> {
    fn drop(&mut self) {
        self.window_requests.closed.store(true, Ordering::Relaxed);
        self.event_loop_awakener.ping();
    }
}

impl<T: WindowType> rwh_06::HasWindowHandle for Window<T> {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::WaylandWindowHandle::new({
            let ptr = self.window.wl_surface().id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_surface will never be null")
        });

        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw.into())) }
    }
}

impl<T: WindowType> rwh_06::HasDisplayHandle for Window<T> {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::WaylandDisplayHandle::new({
            let ptr = self.display.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_proxy should never be null")
        });

        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw.into())) }
    }
}

impl<T: WindowType + Send + Sync + Debug + 'static> CoreWindow for Window<T> {
    fn id(&self) -> WindowId {
        self.window_id
    }

    fn request_redraw(&self) {
        // NOTE: try to not wake up the loop when the event was already scheduled and not yet
        // processed by the loop, because if at this point the value was `true` it could only
        // mean that the loop still haven't dispatched the value to the client and will do
        // eventually, resetting it to `false`.
        if self
            .window_requests
            .redraw_requested
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            self.event_loop_awakener.ping();
        }
    }

    #[inline]
    fn title(&self) -> String {
        self.window_state.lock().unwrap().title().to_owned()
    }

    fn pre_present_notify(&self) {
        self.window_state.lock().unwrap().request_frame_callback();
    }

    fn reset_dead_keys(&self) {
        winit_common::xkb::reset_dead_keys()
    }

    fn surface_position(&self) -> PhysicalPosition<i32> {
        (0, 0).into()
    }

    fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        Err(NotSupportedError::new("window position information is not available on Wayland")
            .into())
    }

    fn set_outer_position(&self, _position: Position) {
        // Not possible.
    }

    fn surface_size(&self) -> PhysicalSize<u32> {
        let window_state = self.window_state.lock().unwrap();
        let scale_factor = window_state.scale_factor();
        super::logical_to_physical_rounded(window_state.surface_size(), scale_factor)
    }

    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let mut window_state = self.window_state.lock().unwrap();
        let new_size = window_state.request_surface_size(size);
        self.request_redraw();
        Some(new_size)
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        let window_state = self.window_state.lock().unwrap();
        let scale_factor = window_state.scale_factor();
        super::logical_to_physical_rounded(window_state.outer_size(), scale_factor)
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }

    fn set_min_surface_size(&self, min_size: Option<Size>) {
        T::set_min_surface_size(self, min_size);
    }

    /// Set the maximum surface size for the window.
    #[inline]
    fn set_max_surface_size(&self, max_size: Option<Size>) {
        T::set_max_surface_size(self, max_size);
    }

    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    fn set_surface_resize_increments(&self, _increments: Option<Size>) {
        warn!("`set_surface_resize_increments` is not implemented for Wayland");
    }

    fn set_title(&self, title: &str) {
        T::set_title(&self.window_state, title);
    }

    #[inline]
    fn set_transparent(&self, transparent: bool) {
        self.window_state.lock().unwrap().set_transparent(transparent);
    }

    fn set_visible(&self, _visible: bool) {
        // Not possible on Wayland.
    }

    fn is_visible(&self) -> Option<bool> {
        None
    }

    fn set_resizable(&self, resizable: bool) {
        T::set_resizable(self, resizable);
    }

    fn is_resizable(&self) -> bool {
        self.window_state.lock().unwrap().resizable()
    }

    fn set_enabled_buttons(&self, _buttons: WindowButtons) {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
    }

    fn enabled_buttons(&self) -> WindowButtons {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
        WindowButtons::all()
    }

    fn set_minimized(&self, minimized: bool) {
        self.window.set_minimized(minimized);
    }

    fn is_minimized(&self) -> Option<bool> {
        // XXX clients don't know whether they are minimized or not.
        None
    }

    fn set_maximized(&self, maximized: bool) {
        self.window.set_maximized(maximized);
    }

    fn is_maximized(&self) -> bool {
        T::is_maximized(&self.window_state)
    }

    fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        self.window.set_fullscreen(fullscreen);
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        T::fullscreen(self)
    }

    #[inline]
    fn scale_factor(&self) -> f64 {
        self.window_state.lock().unwrap().scale_factor()
    }

    #[inline]
    fn set_blur(&self, blur: bool) {
        self.window_state.lock().unwrap().set_blur(blur);
    }

    #[inline]
    fn set_decorations(&self, decorate: bool) {
        T::set_decorations(&self.window_state, decorate);
    }

    #[inline]
    fn is_decorated(&self) -> bool {
        T::is_decorated(&self.window_state)
    }

    fn set_window_level(&self, _level: WindowLevel) {}

    fn set_window_icon(&self, window_icon: Option<winit_core::icon::Icon>) {
        T::set_window_icon(&self.window_state, window_icon);
    }

    #[inline]
    fn request_ime_update(&self, request: ImeRequest) -> Result<(), ImeRequestError> {
        let state_changed = self.window_state.lock().unwrap().request_ime_update(request)?;

        if let Some(allowed) = state_changed {
            let event = WindowEvent::Ime(if allowed { Ime::Enabled } else { Ime::Disabled });
            self.window_events_sink.lock().unwrap().push_window_event(event, self.window_id);
            self.event_loop_awakener.ping();
        }

        Ok(())
    }

    #[inline]
    fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        self.window_state.lock().unwrap().ime_allowed()
    }

    fn focus_window(&self) {}

    fn has_focus(&self) -> bool {
        self.window_state.lock().unwrap().has_focus()
    }

    fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let xdg_activation = match self.xdg_activation.as_ref() {
            Some(xdg_activation) => xdg_activation,
            None => {
                warn!("`request_user_attention` isn't supported");
                return;
            },
        };

        // Urgency is only removed by the compositor and there's no need to raise urgency when it
        // was already raised.
        if request_type.is_none() || self.attention_requested.load(Ordering::Relaxed) {
            return;
        }

        self.attention_requested.store(true, Ordering::Relaxed);
        let surface = self.surface().clone();
        let data = XdgActivationTokenData::Attention((
            surface.clone(),
            Arc::downgrade(&self.attention_requested),
        ));
        let xdg_activation_token = xdg_activation.get_activation_token(&self.queue_handle, data);
        xdg_activation_token.set_surface(&surface);
        xdg_activation_token.commit();
    }

    fn set_theme(&self, theme: Option<Theme>) {
        self.window_state.lock().unwrap().set_theme(theme)
    }

    fn theme(&self) -> Option<Theme> {
        self.window_state.lock().unwrap().theme()
    }

    fn set_content_protected(&self, _protected: bool) {}

    fn set_cursor(&self, cursor: Cursor) {
        let window_state = &mut self.window_state.lock().unwrap();

        match cursor {
            Cursor::Icon(icon) => window_state.set_cursor(icon),
            Cursor::Custom(cursor) => window_state.set_custom_cursor(cursor),
        }
    }

    fn set_cursor_position(&self, position: Position) -> Result<(), RequestError> {
        let scale_factor = self.scale_factor();
        let position = position.to_logical(scale_factor);
        self.window_state
            .lock()
            .unwrap()
            .set_cursor_position(position)
            // Request redraw on success, since the state is double buffered.
            .map(|_| self.request_redraw())
    }

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), RequestError> {
        self.window_state.lock().unwrap().set_cursor_grab(mode)
    }

    fn set_cursor_visible(&self, visible: bool) {
        self.window_state.lock().unwrap().set_cursor_visible(visible);
    }

    fn drag_window(&self) -> Result<(), RequestError> {
        T::drag_window(&self.window_state)
    }

    fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), RequestError> {
        T::drag_resize_window(&self.window_state, direction)
    }

    fn show_window_menu(&self, position: Position) {
        T::show_window_menu(&self.window_state, position);
    }

    fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError> {
        let surface = self.window.wl_surface();

        if hittest {
            surface.set_input_region(None);
            Ok(())
        } else {
            let region = Region::new(&*self.compositor).map_err(|err| os_error!(err))?;
            region.add(0, 0, 0, 0);
            surface.set_input_region(Some(region.wl_region()));
            Ok(())
        }
    }

    fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        let data = self.window.wl_surface().data::<SurfaceData>()?;
        data.outputs()
            .next()
            .map(MonitorHandle::new)
            .map(|monitor| CoreMonitorHandle(Arc::new(monitor)))
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(
            self.monitors
                .lock()
                .unwrap()
                .clone()
                .into_iter()
                .map(|inner| CoreMonitorHandle(Arc::new(inner))),
        )
    }

    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        // NOTE: There's no such concept on Wayland.
        None
    }

    /// Get the raw-window-handle v0.6 display handle.
    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }

    /// Get the raw-window-handle v0.6 window handle.
    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        self
    }
}

/// The request from the window to the event loop.
#[derive(Debug)]
pub struct WindowRequests {
    /// The window was closed.
    pub closed: AtomicBool,

    /// Redraw Requested.
    pub redraw_requested: AtomicBool,
}

impl WindowRequests {
    pub fn take_closed(&self) -> bool {
        self.closed.swap(false, Ordering::Relaxed)
    }

    pub fn take_redraw_requested(&self) -> bool {
        self.redraw_requested.swap(false, Ordering::Relaxed)
    }
}
