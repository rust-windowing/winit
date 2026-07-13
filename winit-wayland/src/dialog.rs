use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use dpi::{PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use rwh_06::RawWindowHandle;
use sctk::shell::WaylandSurface;
use sctk::shell::xdg::dialog::Dialog as SctkDialog;
use sctk::shell::xdg::window::WindowDecorations;
use wayland_client::Proxy;
use wayland_client::protocol::wl_display::WlDisplay;
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event::{Ime, WindowEvent};
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindow, WindowAttributes, WindowButtons, WindowId,
    WindowLevel,
};

use super::ActiveEventLoop;
use crate::WindowAttributesWayland;
use crate::window::Handles;
use crate::window::handles::WindowRequests;
use crate::window::state::{WindowState, WindowType};

#[derive(Debug)]
pub struct Dialog {
    dialog_state: Weak<Mutex<WindowState>>,

    window_id: WindowId,

    /// The wayland display used solely for raw window handle.
    #[allow(dead_code)]
    display: WlDisplay,

    handles: Handles,
}

impl Dialog {
    pub(crate) fn new(
        event_loop_window_target: &ActiveEventLoop,
        mut attributes: WindowAttributes,
    ) -> Result<Self, RequestError> {
        fn error(message: &'static str) -> RequestError {
            RequestError::NotSupported(NotSupportedError::new(message))
        }

        let modal = matches!(attributes.window_type, winit_core::window::WindowType::Dialog {
            modal: true,
            ..
        });
        let queue_handle = event_loop_window_target.queue_handle.clone();
        let mut state = event_loop_window_target.state.borrow_mut();
        let monitors = state.monitors.clone();
        let xdg_activation =
            state.xdg_activation.as_ref().map(|activation_state| activation_state.global().clone());
        let parent_window_handle =
            attributes.parent_window().ok_or(error("Popup without a parent is not supported!"))?;

        let RawWindowHandle::Wayland(parent_window_handle) = parent_window_handle else {
            return Err(error("A Popup requires a parent wayland window handle"));
        };

        let (dialog, dialog_state) = {
            let windows = state.windows.borrow();
            let parent_window_id =
                WindowId::from_raw(parent_window_handle.surface.as_ptr() as usize);
            let Some(parent_window_state) = windows.get(&parent_window_id) else {
                return Err(error("Invalid parent id"));
            };
            let mut parent_window_state = parent_window_state.lock().unwrap();
            let parent_xdg_toplevel = {
                match &parent_window_state.window {
                    WindowType::Window { window, .. } => window.xdg_toplevel().clone(),
                    WindowType::Dialog { dialog, .. } => dialog.xdg_toplevel().clone(),
                    WindowType::Popup { .. } => {
                        return Err(error("Parent of a dialog must be a window or a dialog"));
                    },
                }
            };

            // We prefer server side decorations, however to not have decorations we ask for client
            // side decorations instead.
            let default_decorations = if attributes.decorations {
                WindowDecorations::RequestServer
            } else {
                WindowDecorations::RequestClient
            };

            let surface = state.compositor_state.create_surface(&queue_handle);
            let dialog = state
                .xdg_shell
                .create_dialog(
                    surface.clone(),
                    default_decorations,
                    &queue_handle,
                    &parent_xdg_toplevel,
                )
                .map_err(|_| error("Failed to create dialog"))?;
            parent_window_state.add_child(super::make_wid(dialog.wl_surface()));
            let scale_factor = parent_window_state.scale_factor();
            drop(parent_window_state);

            let dialog_state = WindowState::new(
                event_loop_window_target,
                &state,
                attributes.surface_size.ok_or(error("Invalid size for dialog"))?,
                WindowType::Dialog { dialog: dialog.clone(), last_configure: None },
                attributes.preferred_theme,
                false,
                scale_factor,
                Some(parent_window_id),
            );

            let WindowAttributesWayland { activation_token, .. } = *attributes
                .platform
                .take()
                .and_then(|p| p.cast::<WindowAttributesWayland>().ok())
                .unwrap_or_default();

            // Activate the window when the token is passed.
            if let (Some(xdg_activation), Some(token)) = (xdg_activation.as_ref(), activation_token)
            {
                xdg_activation.activate(token.into_raw(), &surface);
            }
            dialog.set_modal(modal);

            // Do initial commit
            dialog.commit();

            let dialog_state = Arc::new(Mutex::new(dialog_state));
            (dialog, dialog_state)
        };

        let window_id = super::make_wid(dialog.wl_surface());
        state.windows.get_mut().insert(window_id, dialog_state.clone());
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
        while !dialog_state.lock().unwrap().is_configured() {
            event_queue.blocking_dispatch(&mut state).map_err(|err| os_error!(err))?;
            // The compositor may dismiss a popup (e.g. invalid grab serial) by sending
            // popup_done before configure. Detect that and bail out instead of looping forever.
            if state
                .window_compositor_updates
                .iter()
                .any(|u| u.window_id == window_id && u.close_window)
            {
                return Err(error("Popup was dismissed by the compositor before configure"));
            }
        }

        // Wake-up event loop, so it'll send initial redraw requested.
        let event_loop_awakener = event_loop_window_target.event_loop_awakener.clone();
        event_loop_awakener.ping();

        Ok(Self {
            dialog_state: Arc::downgrade(&dialog_state),
            window_id,
            display: event_loop_window_target.handle.connection.display().clone(),
            handles: Handles {
                queue_handle,
                window_requests,
                monitors,
                event_loop_awakener,
                window_events_sink,
                xdg_activation,
                attention_requested: Arc::new(AtomicBool::new(false)),
                compositor: state.compositor_state.clone(),
            },
        })
    }
}

impl CoreWindow for Dialog {
    fn id(&self) -> WindowId {
        self.window_id
    }

    fn request_redraw(&self) {
        self.handles.request_redraw();
    }

    #[inline]
    fn title(&self) -> String {
        let Some(s) = self.dialog_state.upgrade() else { return String::new() };
        s.lock().unwrap().title().to_owned()
    }

    fn pre_present_notify(&self) {
        let Some(s) = self.dialog_state.upgrade() else { return };
        s.lock().unwrap().request_frame_callback();
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

    fn set_outer_position(&self, position: Position) {
        let Some(s) = self.dialog_state.upgrade() else { return };
        let state = s.lock().unwrap();
        if let WindowType::Popup { popup, positioner, .. } = &state.window {
            let position = position.to_logical(state.scale_factor());
            positioner.set_offset(position.x, position.y);
            popup.reposition(positioner, 0);
        }
    }

    fn surface_size(&self) -> PhysicalSize<u32> {
        let Some(s) = self.dialog_state.upgrade() else { return PhysicalSize::default() };
        let dialog_state = s.lock().unwrap();
        let scale_factor = dialog_state.scale_factor();
        super::logical_to_physical_rounded(dialog_state.surface_size(), scale_factor)
    }

    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let s = self.dialog_state.upgrade()?;
        let mut dialog_state = s.lock().unwrap();
        let new_size = dialog_state.request_surface_size(size);
        self.request_redraw();
        Some(new_size)
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        let Some(s) = self.dialog_state.upgrade() else { return PhysicalSize::default() };
        let dialog_state = s.lock().unwrap();
        let scale_factor = dialog_state.scale_factor();
        super::logical_to_physical_rounded(dialog_state.outer_size(), scale_factor)
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }

    fn set_min_surface_size(&self, min_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let min_size = min_size.map(|size| size.to_logical(scale_factor));
        let Some(s) = self.dialog_state.upgrade() else { return };
        s.lock().unwrap().set_min_surface_size(min_size);
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    /// Set the maximum surface size for the window.
    #[inline]
    fn set_max_surface_size(&self, max_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let max_size = max_size.map(|size| size.to_logical(scale_factor));
        let Some(s) = self.dialog_state.upgrade() else { return };
        s.lock().unwrap().set_max_surface_size(max_size);
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        let s = self.dialog_state.upgrade()?;
        s.lock().unwrap().surface_resize_increments()
    }

    fn set_surface_resize_increments(&self, increments: Option<Size>) {
        let Some(s) = self.dialog_state.upgrade() else { return };
        let mut dialog_state = s.lock().unwrap();
        dialog_state.set_surface_resize_increments(increments);
    }

    fn set_title(&self, title: &str) {
        let Some(s) = self.dialog_state.upgrade() else { return };
        s.lock().unwrap().set_title(title.to_owned());
    }

    #[inline]
    fn set_transparent(&self, transparent: bool) {
        let Some(s) = self.dialog_state.upgrade() else { return };
        s.lock().unwrap().set_transparent(transparent);
    }

    fn set_visible(&self, _visible: bool) {
        // Not possible on Wayland.
    }

    fn is_visible(&self) -> Option<bool> {
        None
    }

    fn set_resizable(&self, _resizable: bool) {
        // A popup cannot be resized with the mouse
    }

    fn is_resizable(&self) -> bool {
        // A popup cannot be resized with the mouse
        false
    }

    fn set_enabled_buttons(&self, _buttons: WindowButtons) {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
    }

    fn enabled_buttons(&self) -> WindowButtons {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
        WindowButtons::all()
    }

    fn set_minimized(&self, _minimized: bool) {
        // Not possible for popups
    }

    fn is_minimized(&self) -> Option<bool> {
        // XXX clients don't know whether they are minimized or not.
        None
    }

    fn set_maximized(&self, _maximized: bool) {
        // Not possible for popups
    }

    fn is_maximized(&self) -> bool {
        // Not possible for popups
        false
    }

    fn set_fullscreen(&self, _fullscreen: Option<Fullscreen>) {
        // Not possible for popups
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        None
    }

    #[inline]
    fn scale_factor(&self) -> f64 {
        let Some(s) = self.dialog_state.upgrade() else { return 1.0 };
        s.lock().unwrap().scale_factor()
    }

    #[inline]
    fn set_blur(&self, blur: bool) {
        let Some(s) = self.dialog_state.upgrade() else { return };
        if s.lock().unwrap().set_blur(blur) {
            self.request_redraw();
        }
    }

    #[inline]
    fn set_decorations(&self, _decorate: bool) {
        // Popup does not support decorations
    }

    #[inline]
    fn is_decorated(&self) -> bool {
        // Popup does not support decorations
        false
    }

    fn set_window_level(&self, _level: WindowLevel) {
        // Popup does not have a window level
    }

    fn set_window_icon(&self, _window_icon: Option<winit_core::icon::Icon>) {
        // Popup does not have a window icon
    }

    #[inline]
    fn request_ime_update(&self, request: ImeRequest) -> Result<(), ImeRequestError> {
        let Some(s) = self.dialog_state.upgrade() else { return Ok(()) };
        let state_changed = s.lock().unwrap().request_ime_update(request)?;

        if let Some(allowed) = state_changed {
            let event = WindowEvent::Ime(if allowed { Ime::Enabled } else { Ime::Disabled });
            self.handles.push_window_event(event, self.window_id);
        }

        Ok(())
    }

    #[inline]
    fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        let s = self.dialog_state.upgrade()?;
        s.lock().unwrap().ime_allowed()
    }

    fn focus_window(&self) {}

    fn has_focus(&self) -> bool {
        let Some(s) = self.dialog_state.upgrade() else { return false };
        s.lock().unwrap().has_focus()
    }

    fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        if let Some(state) = self.dialog_state.upgrade() {
            let state = state.lock().unwrap();
            let surface = state.window.wl_surface();
            self.handles.request_user_attention(surface, request_type);
        }
    }

    fn set_theme(&self, _theme: Option<Theme>) {
        // A popup does not have a frame
    }

    fn theme(&self) -> Option<Theme> {
        // A popup does not have a frame
        None
    }

    fn set_content_protected(&self, _protected: bool) {}

    fn set_cursor(&self, cursor: Cursor) {
        let Some(s) = self.dialog_state.upgrade() else { return };
        let mut dialog_state = s.lock().unwrap();
        dialog_state.set_cursor(cursor);
    }

    fn set_cursor_position(&self, position: Position) -> Result<(), RequestError> {
        self.dialog_state
            .upgrade()
            .ok_or(RequestError::Ignored)?
            .lock()
            .unwrap()
            .set_cursor_position(position)
            // Request redraw on success, since the state is double buffered.
            .map(|_| self.request_redraw())
    }

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), RequestError> {
        let Some(s) = self.dialog_state.upgrade() else { return Err(RequestError::Ignored) };
        s.lock().unwrap().set_cursor_grab(mode)
    }

    fn set_cursor_visible(&self, visible: bool) {
        let Some(s) = self.dialog_state.upgrade() else { return };
        s.lock().unwrap().set_cursor_visible(visible);
    }

    fn drag_window(&self) -> Result<(), RequestError> {
        // Popup does not support dragging
        Err(RequestError::Ignored)
    }

    fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), RequestError> {
        // Popup does not support dragging
        Err(RequestError::Ignored)
    }

    fn show_window_menu(&self, _position: Position) {
        // A popup does not have a menu
    }

    fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError> {
        let Some(state) = self.dialog_state.upgrade() else {
            return Err(RequestError::Ignored);
        };

        self.handles.set_cursor_hittest(state.lock().unwrap().window.wl_surface(), hittest)
    }

    fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        self.dialog_state.upgrade()?.lock().ok()?.current_monitor()
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        self.handles.available_monitors()
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

impl Drop for Dialog {
    fn drop(&mut self) {
        self.handles.window_requests.closed.store(true, Ordering::Relaxed);
        self.handles.event_loop_awakener.ping();
    }
}

impl rwh_06::HasWindowHandle for Dialog {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let state = self.dialog_state.upgrade().ok_or(rwh_06::HandleError::Unavailable)?;
        let raw = rwh_06::WaylandWindowHandle::new({
            let ptr = state.lock().unwrap().window.wl_surface().id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_surface will never be null")
        });

        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw.into())) }
    }
}

impl rwh_06::HasDisplayHandle for Dialog {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        if self.dialog_state.upgrade().is_none() {
            return Err(rwh_06::HandleError::Unavailable);
        };
        let raw = rwh_06::WaylandDisplayHandle::new({
            let ptr = self.display.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_proxy should never be null")
        });

        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw.into())) }
    }
}
