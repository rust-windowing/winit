//! The Wayland window.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};

use sctk::reexports::calloop;
use sctk::reexports::client::protocol::wl_display::WlDisplay;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Proxy;
use sctk::reexports::client::QueueHandle;

use sctk::compositor::{CompositorState, Region, SurfaceData};
use sctk::reexports::protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;
use sctk::shell::xdg::window::Window as SctkWindow;
use sctk::shell::xdg::window::WindowDecorations;
use sctk::shell::WaylandSurface;

use crate::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOsError};
use crate::event::{Ime, WindowEvent};
use crate::platform_impl::{
    Fullscreen, MonitorHandle as PlatformMonitorHandle, OsError,
    PlatformSpecificWindowBuilderAttributes as PlatformAttributes,
};
use crate::window::{
    CursorGrabMode, CursorIcon, ImePurpose, ResizeDirection, Theme, UserAttentionType,
    WindowAttributes, WindowButtons,
};

use super::event_loop::sink::EventSink;
use super::output::MonitorHandle;
use super::state::WinitState;
use super::types::xdg_activation::XdgActivationTokenData;
use super::{EventLoopWindowTarget, WindowId};

mod state;

pub use state::WindowState;

/// The Wayland window.
pub struct Window {
    /// Reference to the underlying SCTK window.
    window: SctkWindow,

    /// Window id.
    window_id: WindowId,

    /// The state of the window.
    window_state: Arc<Mutex<WindowState>>,

    /// Compositor to handle WlRegion stuff.
    compositor: Arc<CompositorState>,

    /// The wayland display used solely for raw window handle.
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

    /// The event sink to deliver sythetic events.
    window_events_sink: Arc<Mutex<EventSink>>,
}

impl Window {
    pub(crate) fn new<T>(
        event_loop_window_target: &EventLoopWindowTarget<T>,
        attributes: WindowAttributes,
        platform_attributes: PlatformAttributes,
    ) -> Result<Self, RootOsError> {
        let queue_handle = event_loop_window_target.queue_handle.clone();
        let mut state = event_loop_window_target.state.borrow_mut();

        let monitors = state.monitors.clone();

        let surface = state.compositor_state.create_surface(&queue_handle);
        let compositor = state.compositor_state.clone();
        let xdg_activation = state
            .xdg_activation
            .as_ref()
            .map(|activation_state| activation_state.global().clone());
        let display = event_loop_window_target.connection.display();

        // XXX The initial scale factor must be 1, but it might cause sizing issues on HiDPI.
        let size: LogicalSize<u32> = attributes
            .inner_size
            .map(|size| size.to_logical::<u32>(1.))
            .unwrap_or((800, 600).into());

        // We prefer server side decorations, however to not have decorations we ask for client
        // side decorations instead.
        let default_decorations = if attributes.decorations {
            WindowDecorations::RequestServer
        } else {
            WindowDecorations::RequestClient
        };

        let window =
            state
                .xdg_shell
                .create_window(surface.clone(), default_decorations, &queue_handle);

        let mut window_state = WindowState::new(
            event_loop_window_target.connection.clone(),
            &event_loop_window_target.queue_handle,
            &state,
            size,
            window.clone(),
            attributes.preferred_theme,
        );

        // Set transparency hint.
        window_state.set_transparent(attributes.transparent);

        // Set the decorations hint.
        window_state.set_decorate(attributes.decorations);

        // Set the app_id.
        if let Some(name) = platform_attributes.name.map(|name| name.general) {
            window.set_app_id(name);
        }

        // Set the window title.
        window_state.set_title(attributes.title);

        // Set the min and max sizes.
        let min_size = attributes.min_inner_size.map(|size| size.to_logical(1.));
        let max_size = attributes.max_inner_size.map(|size| size.to_logical(1.));
        window_state.set_min_inner_size(min_size);
        window_state.set_max_inner_size(max_size);

        // Non-resizable implies that the min and max sizes are set to the same value.
        window_state.set_resizable(attributes.resizable);

        // Set startup mode.
        match attributes.fullscreen.map(Into::into) {
            Some(Fullscreen::Exclusive(_)) => {
                warn!("`Fullscreen::Exclusive` is ignored on Wayland");
            }
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.and_then(|monitor| match monitor {
                    PlatformMonitorHandle::Wayland(monitor) => Some(monitor.proxy),
                    #[cfg(x11_platform)]
                    PlatformMonitorHandle::X(_) => None,
                });

                window.set_fullscreen(output.as_ref())
            }
            _ if attributes.maximized => window.set_maximized(),
            _ => (),
        };

        // XXX Do initial commit.
        window.commit();

        // Add the window and window requests into the state.
        let window_state = Arc::new(Mutex::new(window_state));
        let window_id = super::make_wid(&surface);
        state
            .windows
            .get_mut()
            .insert(window_id, window_state.clone());

        let window_requests = WindowRequests {
            redraw_requested: AtomicBool::new(true),
            closed: AtomicBool::new(false),
        };
        let window_requests = Arc::new(window_requests);
        state
            .window_requests
            .get_mut()
            .insert(window_id, window_requests.clone());

        // Setup the event sync to insert `WindowEvents` right from the window.
        let window_events_sink = state.window_events_sink.clone();

        let mut wayland_source = event_loop_window_target.wayland_dispatcher.as_source_mut();
        let event_queue = wayland_source.queue();

        // Do a roundtrip.
        event_queue.roundtrip(&mut state).map_err(|_| {
            os_error!(OsError::WaylandMisc(
                "failed to do initial roundtrip for the window."
            ))
        })?;

        // XXX Wait for the initial configure to arrive.
        while !window_state.lock().unwrap().is_configured() {
            event_queue.blocking_dispatch(&mut state).map_err(|_| {
                os_error!(OsError::WaylandMisc(
                    "failed to dispatch queue while waiting for initial configure."
                ))
            })?;
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

impl Window {
    #[inline]
    pub fn id(&self) -> WindowId {
        self.window_id
    }

    #[inline]
    pub fn set_title(&self, title: impl ToString) {
        let new_title = title.to_string();
        self.window_state.lock().unwrap().set_title(new_title);
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

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let window_state = self.window_state.lock().unwrap();
        let scale_factor = window_state.scale_factor();
        window_state.inner_size().to_physical(scale_factor)
    }

    #[inline]
    pub fn request_redraw(&self) {
        self.window_requests
            .redraw_requested
            .store(true, Ordering::Relaxed);
        self.event_loop_awakener.ping();
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        let window_state = self.window_state.lock().unwrap();
        let scale_factor = window_state.scale_factor();
        window_state.outer_size().to_physical(scale_factor)
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        // TODO should we issue the resize event? I don't think other platforms do so.
        let mut window_state = self.window_state.lock().unwrap();
        let scale_factor = window_state.scale_factor();
        window_state.resize(size.to_logical::<u32>(scale_factor));

        self.request_redraw();
    }

    /// Set the minimum inner size for the window.
    #[inline]
    pub fn set_min_inner_size(&self, min_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let min_size = min_size.map(|size| size.to_logical(scale_factor));
        self.window_state
            .lock()
            .unwrap()
            .set_min_inner_size(min_size)
    }

    /// Set the maximum inner size for the window.
    #[inline]
    pub fn set_max_inner_size(&self, max_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let max_size = max_size.map(|size| size.to_logical(scale_factor));
        self.window_state
            .lock()
            .unwrap()
            .set_max_inner_size(max_size)
    }

    #[inline]
    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    #[inline]
    pub fn set_resize_increments(&self, _increments: Option<Size>) {
        warn!("`set_resize_increments` is not implemented for Wayland");
    }

    #[inline]
    pub fn set_transparent(&self, transparent: bool) {
        self.window_state
            .lock()
            .unwrap()
            .set_transparent(transparent);
    }

    #[inline]
    pub fn has_focus(&self) -> bool {
        self.window_state.lock().unwrap().has_focus()
    }

    #[inline]
    pub fn is_minimized(&self) -> Option<bool> {
        // XXX clients don't know whether they are minimized or not.
        None
    }

    #[inline]
    pub fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), ExternalError> {
        self.window_state
            .lock()
            .unwrap()
            .drag_resize_window(direction)
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        self.window_state.lock().unwrap().set_resizable(resizable);
    }

    #[inline]
    pub fn is_resizable(&self) -> bool {
        self.window_state.lock().unwrap().resizable()
    }

    #[inline]
    pub fn set_enabled_buttons(&self, _buttons: WindowButtons) {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
    }

    #[inline]
    pub fn enabled_buttons(&self) -> WindowButtons {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
        WindowButtons::all()
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.window_state.lock().unwrap().scale_factor()
    }

    #[inline]
    pub fn set_decorations(&self, decorate: bool) {
        self.window_state.lock().unwrap().set_decorate(decorate)
    }

    #[inline]
    pub fn is_decorated(&self) -> bool {
        self.window_state.lock().unwrap().is_decorated()
    }

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        // You can't unminimize the window on Wayland.
        if !minimized {
            warn!("Unminimizing is ignored on Wayland.");
            return;
        }

        self.window.set_minimized();
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        self.window_state
            .lock()
            .unwrap()
            .last_configure
            .as_ref()
            .map(|last_configure| last_configure.is_maximized())
            .unwrap_or_default()
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        if maximized {
            self.window.set_maximized()
        } else {
            self.window.unset_maximized()
        }
    }

    #[inline]
    pub(crate) fn fullscreen(&self) -> Option<Fullscreen> {
        let is_fullscreen = self
            .window_state
            .lock()
            .unwrap()
            .last_configure
            .as_ref()
            .map(|last_configure| last_configure.is_fullscreen())
            .unwrap_or_default();

        if is_fullscreen {
            let current_monitor = self.current_monitor().map(PlatformMonitorHandle::Wayland);
            Some(Fullscreen::Borderless(current_monitor))
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        match fullscreen {
            Some(Fullscreen::Exclusive(_)) => {
                warn!("`Fullscreen::Exclusive` is ignored on Wayland");
            }
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.and_then(|monitor| match monitor {
                    PlatformMonitorHandle::Wayland(monitor) => Some(monitor.proxy),
                    #[cfg(x11_platform)]
                    PlatformMonitorHandle::X(_) => None,
                });

                self.window.set_fullscreen(output.as_ref())
            }
            None => self.window.unset_fullscreen(),
        }
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        self.window_state.lock().unwrap().set_cursor(cursor);
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        self.window_state
            .lock()
            .unwrap()
            .set_cursor_visible(visible);
    }

    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let xdg_activation = match self.xdg_activation.as_ref() {
            Some(xdg_activation) => xdg_activation,
            None => {
                warn!("`request_user_attention` isn't supported");
                return;
            }
        };

        // Urgency is only removed by the compositor and there's no need to raise urgency when it
        // was already raised.
        if request_type.is_none() || self.attention_requested.load(Ordering::Relaxed) {
            return;
        }

        self.attention_requested.store(true, Ordering::Relaxed);
        let surface = self.surface().clone();
        let data =
            XdgActivationTokenData::new(surface.clone(), Arc::downgrade(&self.attention_requested));
        let xdg_activation_token = xdg_activation.get_activation_token(&self.queue_handle, data);
        xdg_activation_token.set_surface(&surface);
        xdg_activation_token.commit();
    }

    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        self.window_state.lock().unwrap().set_cursor_grab(mode)
    }

    #[inline]
    pub fn set_cursor_position(&self, position: Position) -> Result<(), ExternalError> {
        let scale_factor = self.scale_factor();
        let position = position.to_logical(scale_factor);
        self.window_state
            .lock()
            .unwrap()
            .set_cursor_position(position)
            // Request redraw on success, since the state is double buffered.
            .map(|_| self.request_redraw())
    }

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        self.window_state.lock().unwrap().drag_window()
    }

    #[inline]
    pub fn set_cursor_hittest(&self, hittest: bool) -> Result<(), ExternalError> {
        let surface = self.window.wl_surface();

        if hittest {
            surface.set_input_region(None);
            Ok(())
        } else {
            let region = Region::new(&*self.compositor).map_err(|_| {
                ExternalError::Os(os_error!(OsError::WaylandMisc(
                    "failed to set input region."
                )))
            })?;
            region.add(0, 0, 0, 0);
            surface.set_input_region(Some(region.wl_region()));
            Ok(())
        }
    }

    #[inline]
    pub fn set_ime_cursor_area(&self, position: Position, size: Size) {
        let window_state = self.window_state.lock().unwrap();
        if window_state.ime_allowed() {
            let scale_factor = window_state.scale_factor();
            let position = position.to_logical(scale_factor);
            let size = size.to_logical(scale_factor);
            window_state.set_ime_cursor_area(position, size);
        }
    }

    #[inline]
    pub fn set_ime_allowed(&self, allowed: bool) {
        let mut window_state = self.window_state.lock().unwrap();

        if window_state.ime_allowed() != allowed && window_state.set_ime_allowed(allowed) {
            let event = WindowEvent::Ime(if allowed { Ime::Enabled } else { Ime::Disabled });
            self.window_events_sink
                .lock()
                .unwrap()
                .push_window_event(event, self.window_id);
            self.event_loop_awakener.ping();
        }
    }

    #[inline]
    pub fn set_ime_purpose(&self, purpose: ImePurpose) {
        self.window_state.lock().unwrap().set_ime_purpose(purpose);
    }

    #[inline]
    pub fn display(&self) -> &WlDisplay {
        &self.display
    }

    #[inline]
    pub fn surface(&self) -> &WlSurface {
        self.window.wl_surface()
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        let data = self.window.wl_surface().data::<SurfaceData>()?;
        data.outputs().next().map(MonitorHandle::new)
    }

    #[inline]
    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        self.monitors.lock().unwrap().clone()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<PlatformMonitorHandle> {
        // XXX there's no such concept on Wayland.
        None
    }

    #[inline]
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let mut window_handle = WaylandWindowHandle::empty();
        window_handle.surface = self.window.wl_surface().id().as_ptr() as *mut _;
        RawWindowHandle::Wayland(window_handle)
    }

    #[inline]
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        let mut display_handle = WaylandDisplayHandle::empty();
        display_handle.display = self.display.id().as_ptr() as *mut _;
        RawDisplayHandle::Wayland(display_handle)
    }

    #[inline]
    pub fn set_theme(&self, theme: Option<Theme>) {
        self.window_state.lock().unwrap().set_theme(theme)
    }

    #[inline]
    pub fn theme(&self) -> Option<Theme> {
        self.window_state.lock().unwrap().theme()
    }

    #[inline]
    pub fn title(&self) -> String {
        self.window_state.lock().unwrap().title().to_owned()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.window_requests.closed.store(true, Ordering::Relaxed);
        self.event_loop_awakener.ping();
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
