//! The Wayland window.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use dpi::{LogicalSize, PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use rwh_06::RawWindowHandle;
use sctk::reexports::client::Proxy;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::shell::WaylandSurface;
use sctk::shell::xdg::window::{Window as SctkWindow, WindowDecorations};
use tracing::warn;
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event_loop::AsyncRequestSerial;
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindow, WindowAttributes, WindowButtons, WindowId,
    WindowLevel,
};

use super::ActiveEventLoop;
use super::types::xdg_activation::XdgActivationTokenData;
use crate::window::common::WindowCommon;
use crate::window::state::WindowType;
use crate::{WindowAttributesWayland, output};
pub(crate) mod state;
pub use state::WindowState;
pub(crate) mod handles;
pub use handles::Handles;
use handles::WindowRequests;
pub mod common;

/// The Wayland window.
#[derive(Debug)]
pub struct Window {
    /// Reference to the underlying SCTK window.
    window: SctkWindow,

    /// The state of the window.
    window_state: Arc<Mutex<WindowState>>,

    common: WindowCommon,
}

impl Window {
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

        let WindowAttributesWayland { name: app_name, activation_token, prefer_csd, .. } =
            *attributes
                .platform
                .take()
                .and_then(|p| p.cast::<WindowAttributesWayland>().ok())
                .unwrap_or_default();

        let mut scale_factor = None;
        if let Some(RawWindowHandle::Wayland(handle)) = attributes.parent_window() {
            if let Some(s) =
                state.windows.borrow().get(&WindowId::from_raw(handle.surface.as_ptr() as usize))
            {
                scale_factor = Some(s.lock().unwrap().scale_factor());
            }
        }
        let scale_factor = scale_factor.unwrap_or(1.0);

        let mut window_state = WindowState::new(
            event_loop_window_target,
            &state,
            size,
            state::WindowType::Window { window: window.clone(), last_configure: None },
            attributes.preferred_theme,
            prefer_csd,
            scale_factor,
            None,
        );

        window_state.set_window_icon(attributes.window_icon);

        // Set transparency hint.
        window_state.set_transparent(attributes.transparent);

        // Set blur.
        let _ = window_state.set_blur(attributes.blur);

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

        window_state.set_cursor(attributes.cursor);

        // Apply resize increments.
        if let Some(increments) = attributes.surface_resize_increments {
            let increments = increments.to_logical(window_state.scale_factor());
            window_state.set_resize_increments(Some(increments));
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
        state.windows.get_mut().insert(window_id, window_state.clone());

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

        let state_weak = Arc::downgrade(&window_state);

        Ok(Self {
            window,
            window_state,
            common: WindowCommon {
                state: state_weak,
                window_id,
                display,
                handles: Handles {
                    queue_handle,
                    window_requests,
                    monitors,
                    event_loop_awakener,
                    window_events_sink,

                    compositor,

                    xdg_activation,
                    attention_requested: Arc::new(AtomicBool::new(false)),
                },
            },
        })
    }

    pub(crate) fn xdg_toplevel(&self) -> Option<NonNull<c_void>> {
        NonNull::new(self.window.xdg_toplevel().id().as_ptr().cast())
    }
}

impl Window {
    pub fn request_activation_token(&self) -> Result<AsyncRequestSerial, RequestError> {
        let xdg_activation = match self.common.handles.xdg_activation.as_ref() {
            Some(xdg_activation) => xdg_activation,
            None => return Err(NotSupportedError::new("xdg_activation_v1 is not available").into()),
        };

        let serial = AsyncRequestSerial::get();

        let data = XdgActivationTokenData::Obtain((self.common.id(), serial));
        let xdg_activation_token =
            xdg_activation.get_activation_token(&self.common.handles.queue_handle, data);
        xdg_activation_token.set_surface(self.surface());
        xdg_activation_token.commit();

        Ok(serial)
    }

    #[inline]
    pub fn surface(&self) -> &WlSurface {
        self.window.wl_surface()
    }
}

impl CoreWindow for Window {
    fn window_type(&self) -> winit_core::window::WindowType {
        winit_core::window::WindowType::Window
    }

    fn id(&self) -> WindowId {
        self.common.id()
    }

    fn request_redraw(&self) {
        self.common.request_redraw();
    }

    #[inline]
    fn title(&self) -> String {
        self.common.title()
    }

    fn pre_present_notify(&self) {
        self.common.pre_present_notify();
    }

    fn reset_dead_keys(&self) {
        self.common.reset_dead_keys();
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
        self.common.surface_size()
    }

    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        self.common.request_surface_size(size)
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        self.common.outer_size()
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        self.common.safe_area()
    }

    fn set_min_surface_size(&self, min_size: Option<Size>) {
        self.common.set_min_surface_size(min_size);
    }

    /// Set the maximum surface size for the window.
    #[inline]
    fn set_max_surface_size(&self, max_size: Option<Size>) {
        self.common.set_max_surface_size(max_size);
    }

    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        self.common.surface_resize_increments()
    }

    fn set_surface_resize_increments(&self, increments: Option<Size>) {
        self.common.set_surface_resize_increments(increments);
    }

    fn set_title(&self, title: &str) {
        self.common.set_title(title);
    }

    #[inline]
    fn set_transparent(&self, transparent: bool) {
        self.common.set_transparent(transparent);
    }

    fn set_visible(&self, visible: bool) {
        self.common.set_visible(visible);
    }

    fn is_visible(&self) -> Option<bool> {
        self.common.is_visible()
    }

    fn set_resizable(&self, resizable: bool) {
        if self.window_state.lock().unwrap().set_resizable(resizable) {
            // NOTE: Requires commit to be applied.
            self.request_redraw();
        }
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
        // You can't unminimize the window on Wayland.
        if !minimized {
            warn!("Unminimizing is ignored on Wayland.");
            return;
        }

        self.window.set_minimized();
    }

    fn is_minimized(&self) -> Option<bool> {
        // XXX clients don't know whether they are minimized or not.
        None
    }

    fn set_maximized(&self, maximized: bool) {
        if maximized { self.window.set_maximized() } else { self.window.unset_maximized() }
    }

    fn is_maximized(&self) -> bool {
        if let WindowType::Window { last_configure, .. } = &self.window_state.lock().unwrap().window
        {
            last_configure
                .as_ref()
                .map(|last_configure| last_configure.is_maximized())
                .unwrap_or_default()
        } else {
            false
        }
    }

    fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        match fullscreen {
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.as_ref().and_then(|monitor| {
                    monitor.cast_ref::<output::MonitorHandle>().map(|handle| &handle.proxy)
                });

                self.window.set_fullscreen(output)
            },
            Some(_) => {
                warn!("this fullscreen mode is ignored on Wayland");
            },
            None => self.window.unset_fullscreen(),
        }
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        let is_fullscreen = if let WindowType::Window { last_configure, .. } =
            &self.window_state.lock().unwrap().window
        {
            last_configure
                .as_ref()
                .map(|last_configure| last_configure.is_fullscreen())
                .unwrap_or_default()
        } else {
            false
        };

        if is_fullscreen {
            let current_monitor = self.current_monitor();
            Some(Fullscreen::Borderless(current_monitor))
        } else {
            None
        }
    }

    #[inline]
    fn scale_factor(&self) -> f64 {
        self.common.scale_factor()
    }

    #[inline]
    fn set_blur(&self, blur: bool) {
        self.common.set_blur(blur);
    }

    #[inline]
    fn set_decorations(&self, decorate: bool) {
        self.common.set_decorations(decorate);
    }

    #[inline]
    fn is_decorated(&self) -> bool {
        self.common.is_decorated().unwrap()
    }

    fn set_window_level(&self, level: WindowLevel) {
        self.common.set_window_level(level);
    }

    fn set_window_icon(&self, window_icon: Option<winit_core::icon::Icon>) {
        self.common.set_window_icon(window_icon);
    }

    #[inline]
    fn request_ime_update(&self, request: ImeRequest) -> Result<(), ImeRequestError> {
        self.common.request_ime_update(request)
    }

    #[inline]
    fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        self.common.ime_capabilities()
    }

    fn focus_window(&self) {}

    fn has_focus(&self) -> bool {
        self.common.has_focus()
    }

    fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        self.common.request_user_attention(request_type);
    }

    fn set_theme(&self, theme: Option<Theme>) {
        self.common.set_theme(theme);
    }

    fn theme(&self) -> Option<Theme> {
        self.common.theme()
    }

    fn set_content_protected(&self, protected: bool) {
        self.common.set_content_protected(protected);
    }

    fn set_cursor(&self, cursor: Cursor) {
        self.common.set_cursor(cursor);
    }

    fn set_cursor_position(&self, position: Position) -> Result<(), RequestError> {
        self.common.set_cursor_position(position)
    }

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), RequestError> {
        self.common.set_cursor_grab(mode)
    }

    fn set_cursor_visible(&self, visible: bool) {
        self.common.set_cursor_visible(visible);
    }

    fn drag_window(&self) -> Result<(), RequestError> {
        self.common.drag_window()
    }

    fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), RequestError> {
        self.common.drag_resize_window(direction)
    }

    fn show_window_menu(&self, position: Position) {
        self.common.show_window_menu(position);
    }

    fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError> {
        self.common.set_cursor_hittest(hittest)
    }

    fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        self.common.current_monitor()
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        self.common.available_monitors()
    }

    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        self.common.primary_monitor()
    }

    /// Get the raw-window-handle v0.6 display handle.
    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        &self.common
    }

    /// Get the raw-window-handle v0.6 window handle.
    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        &self.common
    }
}
