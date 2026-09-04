use std::sync::atomic::Ordering;
use std::sync::{Mutex, Weak};

use dpi::{PhysicalInsets, PhysicalSize, Position, Size};
use sctk::shell::WaylandSurface;
use wayland_client::Proxy;
use wayland_client::protocol::wl_display::WlDisplay;
use winit_core::cursor::Cursor;
use winit_core::error::RequestError;
use winit_core::event::{Ime, WindowEvent};
use winit_core::monitor::MonitorHandle as CoreMonitorHandle;
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, WindowId, WindowLevel,
};

use crate::window::Handles;
use crate::window::state::WindowState;

#[derive(Debug)]
pub struct WindowCommon {
    /// The state of the window.
    /// The only single truth of the state is stored
    /// in the event loop state, because if the server decides to destroy the popup
    /// we cannot use it anymore
    pub(crate) state: Weak<Mutex<WindowState>>,

    pub(crate) window_id: WindowId,

    /// The wayland display used solely for raw window handle.
    pub(crate) display: WlDisplay,

    /// Common handles like queue, window requests, monitors and so on
    pub(crate) handles: Handles,
}

impl WindowCommon {
    pub(crate) fn id(&self) -> WindowId {
        self.window_id
    }

    pub(crate) fn request_redraw(&self) {
        self.handles.request_redraw();
    }

    #[inline]
    pub(crate) fn title(&self) -> String {
        let Some(s) = self.state.upgrade() else { return String::new() };
        s.lock().unwrap().title().to_owned()
    }

    pub(crate) fn pre_present_notify(&self) {
        let Some(s) = self.state.upgrade() else { return };
        s.lock().unwrap().request_frame_callback();
    }

    pub(crate) fn reset_dead_keys(&self) {
        winit_common::xkb::reset_dead_keys()
    }

    pub(crate) fn surface_size(&self) -> PhysicalSize<u32> {
        let Some(s) = self.state.upgrade() else { return PhysicalSize::default() };
        let s = s.lock().unwrap();
        s.surface_size_physical()
    }

    pub(crate) fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let s = self.state.upgrade()?;
        let mut s = s.lock().unwrap();
        let new_size = s.request_surface_size(size);
        self.request_redraw();
        Some(new_size)
    }

    pub(crate) fn outer_size(&self) -> PhysicalSize<u32> {
        let Some(s) = self.state.upgrade() else { return PhysicalSize::default() };
        let s = s.lock().unwrap();
        s.outer_size_physical()
    }

    pub(crate) fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }

    pub(crate) fn set_min_surface_size(&self, min_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let min_size = min_size.map(|size| size.to_logical(scale_factor));
        let Some(s) = self.state.upgrade() else { return };
        s.lock().unwrap().set_min_surface_size(min_size);
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    /// Set the maximum surface size for the window.
    #[inline]
    pub(crate) fn set_max_surface_size(&self, max_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let max_size = max_size.map(|size| size.to_logical(scale_factor));
        let Some(s) = self.state.upgrade() else { return };
        s.lock().unwrap().set_max_surface_size(max_size);
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    pub(crate) fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        let s = self.state.upgrade()?;
        s.lock().unwrap().surface_resize_increments()
    }

    pub(crate) fn set_surface_resize_increments(&self, increments: Option<Size>) {
        let Some(s) = self.state.upgrade() else { return };
        let mut state = s.lock().unwrap();
        state.set_surface_resize_increments(increments);
    }

    pub(crate) fn set_title(&self, title: &str) {
        let Some(s) = self.state.upgrade() else { return };
        s.lock().unwrap().set_title(title.to_owned());
    }

    #[inline]
    pub(crate) fn set_transparent(&self, transparent: bool) {
        let Some(s) = self.state.upgrade() else { return };
        s.lock().unwrap().set_transparent(transparent);
    }

    pub(crate) fn set_visible(&self, _visible: bool) {
        // Not possible on Wayland.
    }

    pub(crate) fn is_visible(&self) -> Option<bool> {
        None
    }

    #[inline]
    pub(crate) fn scale_factor(&self) -> f64 {
        let Some(s) = self.state.upgrade() else { return 1.0 };
        s.lock().unwrap().scale_factor()
    }

    #[inline]
    pub(crate) fn set_blur(&self, blur: bool) {
        let Some(s) = self.state.upgrade() else { return };
        if s.lock().unwrap().set_blur(blur) {
            self.request_redraw();
        }
    }

    #[inline]
    pub(crate) fn set_decorations(&self, decorate: bool) {
        let Some(s) = self.state.upgrade() else { return };
        s.lock().unwrap().set_decorate(decorate)
    }

    #[inline]
    pub(crate) fn is_decorated(&self) -> Option<bool> {
        let Some(s) = self.state.upgrade() else { return None };
        Some(s.lock().unwrap().is_decorated())
    }

    pub(crate) fn set_window_level(&self, _level: WindowLevel) {}

    pub(crate) fn set_window_icon(&self, window_icon: Option<winit_core::icon::Icon>) {
        let Some(s) = self.state.upgrade() else { return };
        s.lock().unwrap().set_window_icon(window_icon)
    }

    #[inline]
    pub(crate) fn request_ime_update(&self, request: ImeRequest) -> Result<(), ImeRequestError> {
        let Some(s) = self.state.upgrade() else { return Ok(()) };
        let state_changed = s.lock().unwrap().request_ime_update(request)?;

        if let Some(allowed) = state_changed {
            let event = WindowEvent::Ime(if allowed { Ime::Enabled } else { Ime::Disabled });
            self.handles.push_window_event(event, self.window_id);
        }

        Ok(())
    }

    #[inline]
    pub(crate) fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        let s = self.state.upgrade()?;
        s.lock().unwrap().ime_allowed()
    }

    pub(crate) fn focus_window(&self) {}

    pub(crate) fn has_focus(&self) -> bool {
        let Some(s) = self.state.upgrade() else { return false };
        s.lock().unwrap().has_focus()
    }

    pub(crate) fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        if let Some(state) = self.state.upgrade() {
            let state = state.lock().unwrap();
            let surface = state.window.wl_surface();
            self.handles.request_user_attention(surface, request_type);
        }
    }

    pub(crate) fn set_theme(&self, theme: Option<Theme>) {
        let Some(s) = self.state.upgrade() else { return };
        s.lock().unwrap().set_theme(theme)
    }

    pub(crate) fn theme(&self) -> Option<Theme> {
        let Some(s) = self.state.upgrade() else { return None };
        s.lock().unwrap().theme()
    }

    pub(crate) fn set_content_protected(&self, _protected: bool) {}

    pub(crate) fn set_cursor(&self, cursor: Cursor) {
        let Some(s) = self.state.upgrade() else { return };
        let mut state = s.lock().unwrap();
        state.set_cursor(cursor);
    }

    pub(crate) fn set_cursor_position(&self, position: Position) -> Result<(), RequestError> {
        self.state
            .upgrade()
            .ok_or(RequestError::Ignored)?
            .lock()
            .unwrap()
            .set_cursor_position(position)
            // Request redraw on success, since the state is double buffered.
            .map(|_| self.request_redraw())
    }

    pub(crate) fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), RequestError> {
        let Some(s) = self.state.upgrade() else { return Err(RequestError::Ignored) };
        s.lock().unwrap().set_cursor_grab(mode)
    }

    pub(crate) fn set_cursor_visible(&self, visible: bool) {
        let Some(s) = self.state.upgrade() else { return };
        s.lock().unwrap().set_cursor_visible(visible);
    }

    pub(crate) fn drag_window(&self) -> Result<(), RequestError> {
        let Some(s) = self.state.upgrade() else { return Err(RequestError::Ignored) };
        s.lock().unwrap().drag_window()
    }

    pub(crate) fn drag_resize_window(
        &self,
        direction: ResizeDirection,
    ) -> Result<(), RequestError> {
        let Some(s) = self.state.upgrade() else { return Err(RequestError::Ignored) };
        s.lock().unwrap().drag_resize_window(direction)
    }

    pub(crate) fn show_window_menu(&self, position: Position) {
        let Some(s) = self.state.upgrade() else { return };
        let s = s.lock().unwrap();
        let position = position.to_logical(s.scale_factor());
        s.show_window_menu(position);
    }

    pub(crate) fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError> {
        let Some(state) = self.state.upgrade() else {
            return Err(RequestError::Ignored);
        };

        self.handles.set_cursor_hittest(state.lock().unwrap().window.wl_surface(), hittest)
    }

    pub(crate) fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        self.state.upgrade()?.lock().ok()?.current_monitor()
    }

    pub(crate) fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        self.handles.available_monitors()
    }

    pub(crate) fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        // NOTE: There's no such concept on Wayland.
        None
    }
}

impl Drop for WindowCommon {
    fn drop(&mut self) {
        self.handles.window_requests.closed.store(true, Ordering::Relaxed);
        self.handles.event_loop_awakener.ping();
    }
}

impl rwh_06::HasWindowHandle for WindowCommon {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let state = self.state.upgrade().ok_or(rwh_06::HandleError::Unavailable)?;
        let raw = rwh_06::WaylandWindowHandle::new({
            let ptr = state.lock().unwrap().window.wl_surface().id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_surface will never be null")
        });

        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw.into())) }
    }
}

impl rwh_06::HasDisplayHandle for WindowCommon {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        if self.state.upgrade().is_none() {
            return Err(rwh_06::HandleError::Unavailable);
        };
        let raw = rwh_06::WaylandDisplayHandle::new({
            let ptr = self.display.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_proxy should never be null")
        });

        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw.into())) }
    }
}
