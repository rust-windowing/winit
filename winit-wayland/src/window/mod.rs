//! The Wayland window.

use core::fmt;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use dpi::{LogicalSize, PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use rwh_06::RawWindowHandle;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::shell::WaylandSurface;
use sctk::shell::xdg::window::WindowDecorations;
use tracing::warn;
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;
use wayland_protocols::xdg::shell::client::xdg_toplevel;
use winit_core::cursor::Cursor;
use winit_core::error::{CreateWindowError, InvalidInput, NotSupportedError, RequestError};
use winit_core::event_loop::AsyncRequestSerial;
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindow, WindowAttributes, WindowButtons, WindowId,
    WindowLevel,
};

use super::ActiveEventLoop;
use super::types::xdg_activation::XdgActivationTokenData;
use crate::state::WinitState;
use crate::window::common::WindowCommon;
use crate::window::state::{WindowParent, WindowType};
use crate::{WindowAttributesWayland, output};
pub(crate) mod state;
pub use state::WindowState;
pub(crate) mod handles;
pub use handles::Handles;
use handles::WindowRequests;
pub(crate) mod common;

/// The Wayland window.
#[derive(Debug)]
pub struct Window {
    pub(crate) common: WindowCommon,

    /// The window's `wl_surface`. It stays alive as long as this window, because
    /// [`WindowCommon::state`] holds a strong reference to the SCTK window that owns it.
    surface: WlSurface,
}

/// Registers `window_state` with the event loop, waits for its initial configure, and wakes the
/// event loop for the first redraw.
///
/// If `dismissed_error` is set, the wait loop bails out early with that message when the
/// compositor closes the surface before ever sending a configure, and the surface is removed from
/// the event loop again. Use it for popups, where `xdg_popup.popup_done` is final. Pass `None`
/// for a toplevel: `xdg_toplevel.close` is only a request, the toplevel will still be configured,
/// and the queued close is delivered as `WindowEvent::CloseRequested` once the window exists.
pub(crate) fn finish_window_setup(
    event_loop_window_target: &ActiveEventLoop,
    state: &mut WinitState,
    surface: &WlSurface,
    window_state: &Arc<Mutex<WindowState>>,
    dismissed_error: Option<&'static str>,
) -> Result<(WindowId, Arc<WindowRequests>), CreateWindowError> {
    let window_id = super::make_wid(surface);
    state.windows.get_mut().insert(window_id, window_state.clone());

    let window_requests = Arc::new(WindowRequests {
        redraw_requested: AtomicBool::new(true),
        closed: AtomicBool::new(false),
    });
    state.window_requests.get_mut().insert(window_id, window_requests.clone());

    let mut wayland_source = event_loop_window_target.wayland_dispatcher.as_source_mut();
    let event_queue = wayland_source.queue();

    // Do a roundtrip.
    event_queue.roundtrip(state).map_err(|err| os_error!(err))?;

    // XXX Wait for the initial configure to arrive.
    while !window_state.lock().unwrap().is_configured() {
        event_queue.blocking_dispatch(state).map_err(|err| os_error!(err))?;

        // The compositor may dismiss the surface before it ever sends a configure. Detect
        // that and bail out instead of looping forever.
        if let Some(dismissed_error) = dismissed_error
            && state
                .window_compositor_updates
                .iter()
                .any(|u| u.window_id == window_id && u.close_window)
        {
            state.windows.get_mut().remove(&window_id);
            state.window_requests.get_mut().remove(&window_id);
            state.window_compositor_updates.retain(|u| u.window_id != window_id);
            return Err(os_error!(DismissedError(dismissed_error)).into());
        }
    }

    // Wake-up event loop, so it'll send initial redraw requested.
    event_loop_window_target.event_loop_awakener.ping();

    Ok((window_id, window_requests))
}

/// Constructing a window/popup failed: the compositor dismissed the surface before it
/// ever sent an initial configure.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct DismissedError(&'static str);

impl fmt::Display for DismissedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for DismissedError {}

type SetupResults =
    (Arc<Mutex<WindowState>>, WindowId, Arc<WindowRequests>, Option<XdgActivationV1>);

impl Window {
    pub(crate) fn new(
        event_loop_window_target: &ActiveEventLoop,
        attributes: WindowAttributes,
    ) -> Result<Self, CreateWindowError> {
        let queue_handle = event_loop_window_target.queue_handle.clone();
        let mut state = event_loop_window_target.state.borrow_mut();

        let monitors = state.monitors.clone();

        let surface = state.compositor_state.create_surface(&queue_handle);
        let compositor = state.compositor_state.clone();
        let display = event_loop_window_target.handle.connection.display();

        // We prefer server side decorations, however to not have decorations we ask for client
        // side decorations instead.
        let default_decorations = if attributes.decorations {
            WindowDecorations::RequestServer
        } else {
            WindowDecorations::RequestClient
        };

        let window =
            state.xdg_shell.create_window(surface.clone(), default_decorations, &queue_handle);

        let (window_state, window_id, window_requests, xdg_activation) = Self::setup(
            event_loop_window_target,
            &mut state,
            attributes,
            state::WindowType::Window { window, last_configure: None },
            &surface,
        )?;

        let event_loop_awakener = event_loop_window_target.event_loop_awakener.clone();
        // Setup the event sync to insert `WindowEvents` right from the window.
        let window_events_sink = state.window_events_sink.clone();

        Ok(Self {
            common: WindowCommon {
                state: window_state,
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
            surface,
        })
    }

    pub(crate) fn parent_from_attributes(
        state: &mut WinitState,
        attributes: &WindowAttributes,
    ) -> Result<Option<(WindowParent, xdg_toplevel::XdgToplevel, f64)>, CreateWindowError> {
        let Some(parent_window_handle) = attributes.parent_window() else { return Ok(None) };

        let RawWindowHandle::Wayland(parent_window_handle) = parent_window_handle else {
            return Err(
                InvalidInput::new("A Window requires a parent wayland window handle").into()
            );
        };

        let parent_window_id = WindowId::from_raw(parent_window_handle.surface.as_ptr() as usize);

        let (parent_state, parent_xdg_toplevel, scale_factor) = {
            let windows = state.windows.borrow();
            let Some(parent_state) = windows.get(&parent_window_id) else {
                return Err(InvalidInput::new("Unknown parent window id").into());
            };
            let parent_window_state = parent_state.lock().unwrap();
            let scale_factor = parent_window_state.scale_factor();
            let parent_xdg_toplevel = {
                match &parent_window_state.window {
                    WindowType::Window { window, .. } => window.xdg_toplevel().clone(),
                    WindowType::Popup { .. } => {
                        return Err(InvalidInput::new("Parent of a window must be a window").into());
                    },
                }
            };
            (parent_state.clone(), parent_xdg_toplevel, scale_factor)
        };
        let parent = WindowParent { id: parent_window_id, state: parent_state };
        Ok(Some((parent, parent_xdg_toplevel, scale_factor)))
    }

    pub(crate) fn setup(
        event_loop_window_target: &ActiveEventLoop,
        state: &mut WinitState,
        mut attributes: WindowAttributes,
        window_type: state::WindowType,
        surface: &WlSurface,
    ) -> Result<SetupResults, CreateWindowError> {
        let (parent, parent_xdg_toplevel, scale_factor) =
            match Self::parent_from_attributes(state, &attributes)? {
                Some((parent, xdg_toplevel, scale_factor)) => {
                    (Some(parent), Some(xdg_toplevel), scale_factor)
                },
                None => (None, None, 1.0),
            };
        let parent_window_id = parent.as_ref().map(|parent| parent.id);

        let size: Size = attributes.surface_size.unwrap_or(LogicalSize::new(800., 600.).into());
        let WindowAttributesWayland { name: app_name, activation_token, prefer_csd, .. } =
            *attributes
                .platform
                .take()
                .and_then(|p| p.cast::<WindowAttributesWayland>().ok())
                .unwrap_or_default();

        let mut window_state = WindowState::new(
            event_loop_window_target,
            state,
            size,
            window_type,
            attributes.preferred_theme,
            prefer_csd,
            scale_factor,
            parent,
        );

        window_state.set_window_icon(attributes.window_icon);

        // Set transparency hint.
        window_state.set_transparent(attributes.transparent);

        // Set blur.
        let _ = window_state.set_blur(attributes.blur);

        // Set the decorations hint.
        window_state.set_decorate(attributes.decorations);

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
        let xdg_toplevel = window_state.xdg_toplevel().unwrap(); // a window is always an xdg toplevel
        xdg_toplevel.set_parent(parent_xdg_toplevel.as_ref());

        // Set the app_id.
        if let Some(name) = app_name.map(|name| name.general) {
            xdg_toplevel.set_app_id(name);
        }
        match attributes.fullscreen {
            Some(Fullscreen::Exclusive(..)) => {
                warn!("`Fullscreen::Exclusive` is ignored on Wayland");
            },
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.as_ref().and_then(|monitor| {
                    monitor.cast_ref::<output::MonitorHandle>().map(|handle| &handle.proxy)
                });

                xdg_toplevel.set_fullscreen(output)
            },
            _ if attributes.maximized => xdg_toplevel.set_maximized(),
            _ => (),
        };

        window_state.set_cursor(attributes.cursor);

        // Apply resize increments.
        if let Some(increments) = attributes.surface_resize_increments {
            let increments = increments.to_logical(window_state.scale_factor());
            window_state.set_resize_increments(Some(increments));
        }

        // Activate the window when the token is passed.
        let xdg_activation =
            state.xdg_activation.as_ref().map(|activation_state| activation_state.global().clone());
        if let (Some(xdg_activation), Some(token)) = (xdg_activation.as_ref(), activation_token) {
            xdg_activation.activate(token.into_raw(), surface);
        }

        // XXX Do initial commit.
        window_state.window.wl_surface().commit();

        // Add the window and window requests into the state.
        let window_state = Arc::new(Mutex::new(window_state));

        let (window_id, window_requests) = finish_window_setup(
            event_loop_window_target,
            state,
            surface,
            &window_state,
            // `xdg_toplevel.close` is only a request, so it never fails the creation.
            None,
        )?;

        if let Some(parent_window_id) = parent_window_id {
            let windows = state.windows.borrow();
            if let Some(parent_window_state) = windows.get(&parent_window_id) {
                parent_window_state.lock().unwrap().add_child(window_id);
            }
        }

        Ok((window_state, window_id, window_requests, xdg_activation))
    }

    pub(crate) fn xdg_toplevel(&self) -> Option<NonNull<c_void>> {
        self.common.xdg_toplevel()
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
        &self.surface
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
        self.common.set_resizable(resizable);
    }

    fn is_resizable(&self) -> bool {
        self.common.is_resizable()
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

        if let Some(xdg_toplevel) = self.common.state.lock().unwrap().xdg_toplevel() {
            xdg_toplevel.set_minimized();
        }
    }

    fn is_minimized(&self) -> Option<bool> {
        // XXX clients don't know whether they are minimized or not.
        None
    }

    fn set_maximized(&self, maximized: bool) {
        self.common.set_maximized(maximized)
    }

    fn is_maximized(&self) -> bool {
        self.common.is_maximized()
    }

    fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        self.common.set_fullscreen(fullscreen)
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        self.common.fullscreen()
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

    fn focus_window(&self) {
        self.common.focus_window();
    }

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
