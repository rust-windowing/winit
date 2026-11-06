use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use dpi::{PhysicalInsets, PhysicalSize, Position, Size};
use sctk::shell::WaylandSurface;
use wayland_client::Proxy;
use wayland_client::protocol::wl_display::WlDisplay;
use winit_core::cursor::Cursor;
use winit_core::error::RequestError;
use winit_core::event::{Ime, WindowEvent};
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, WindowId, WindowLevel,
};

use crate::window::Handles;
use crate::window::state::WindowState;

#[derive(Debug)]
pub(crate) struct WindowCommon {
    /// Keeps the window's state alive for as long as this handle exists, independently of
    /// whatever the event loop's `windows` map does with its own reference.
    ///
    /// The state, and with it the `wl_surface` that the raw window handle hands out, must live
    /// until this handle is dropped: graphics libraries keep using that pointer for as long as
    /// they hold the window. A window or popup with a parent in turn keeps its parent's state
    /// alive (see [`WindowParent`](crate::window::state::WindowParent)), so a parent is never
    /// destroyed before its children.
    pub(crate) state: Arc<Mutex<WindowState>>,

    pub(crate) window_id: WindowId,

    /// The wayland display used solely for raw window handle.
    pub(crate) display: WlDisplay,

    /// Common handles like queue, window requests, monitors and so on
    pub(crate) handles: Handles,
}

impl WindowCommon {
    /// The window's `xdg_toplevel`, or [`None`] for a popup.
    pub(crate) fn xdg_toplevel(&self) -> Option<NonNull<c_void>> {
        let state = self.state.lock().unwrap();
        NonNull::new(state.xdg_toplevel()?.id().as_ptr().cast())
    }

    pub(crate) fn id(&self) -> WindowId {
        self.window_id
    }

    pub(crate) fn request_redraw(&self) {
        self.handles.request_redraw();
    }

    #[inline]
    pub(crate) fn title(&self) -> String {
        self.state.lock().unwrap().title().to_owned()
    }

    pub(crate) fn pre_present_notify(&self) {
        self.state.lock().unwrap().request_frame_callback();
    }

    pub(crate) fn reset_dead_keys(&self) {
        winit_common::xkb::reset_dead_keys()
    }

    pub(crate) fn surface_size(&self) -> PhysicalSize<u32> {
        let s = self.state.lock().unwrap();
        s.surface_size_physical()
    }

    pub(crate) fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let mut s = self.state.lock().unwrap();
        let new_size = s.request_surface_size(size);
        self.request_redraw();
        Some(new_size)
    }

    pub(crate) fn outer_size(&self) -> PhysicalSize<u32> {
        let s = self.state.lock().unwrap();
        s.outer_size_physical()
    }

    pub(crate) fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }

    pub(crate) fn set_min_surface_size(&self, min_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let min_size = min_size.map(|size| size.to_logical(scale_factor));
        self.state.lock().unwrap().set_min_surface_size(min_size);
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    /// Set the maximum surface size for the window.
    #[inline]
    pub(crate) fn set_max_surface_size(&self, max_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let max_size = max_size.map(|size| size.to_logical(scale_factor));
        self.state.lock().unwrap().set_max_surface_size(max_size);
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    pub(crate) fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        self.state.lock().unwrap().surface_resize_increments()
    }

    pub(crate) fn set_surface_resize_increments(&self, increments: Option<Size>) {
        let mut state = self.state.lock().unwrap();
        state.set_surface_resize_increments(increments);
    }

    pub(crate) fn set_title(&self, title: &str) {
        self.state.lock().unwrap().set_title(title.to_owned());
    }

    #[inline]
    pub(crate) fn set_transparent(&self, transparent: bool) {
        self.state.lock().unwrap().set_transparent(transparent);
    }

    pub(crate) fn set_visible(&self, _visible: bool) {
        // Not possible on Wayland.
    }

    pub(crate) fn is_visible(&self) -> Option<bool> {
        None
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn set_resizable(&self, resizable: bool) {
        if self.state.lock().unwrap().set_resizable(resizable) {
            // NOTE: Requires commit to be applied.
            self.request_redraw();
        }
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn is_resizable(&self) -> bool {
        self.state.lock().unwrap().resizable()
    }

    #[inline]
    pub(crate) fn scale_factor(&self) -> f64 {
        self.state.lock().unwrap().scale_factor()
    }

    #[inline]
    pub(crate) fn set_blur(&self, blur: bool) {
        if self.state.lock().unwrap().set_blur(blur) {
            self.request_redraw();
        }
    }

    /// Only meaningful for a top-level surface.
    #[inline]
    pub(super) fn set_decorations(&self, decorate: bool) {
        self.state.lock().unwrap().set_decorate(decorate)
    }

    /// Only meaningful for a top-level surface.
    #[inline]
    pub(super) fn is_decorated(&self) -> Option<bool> {
        Some(self.state.lock().unwrap().is_decorated())
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn set_window_level(&self, _level: WindowLevel) {}

    /// Only meaningful for a top-level surface.
    pub(super) fn set_window_icon(&self, window_icon: Option<winit_core::icon::Icon>) {
        self.state.lock().unwrap().set_window_icon(window_icon)
    }

    #[inline]
    pub(crate) fn request_ime_update(&self, request: ImeRequest) -> Result<(), ImeRequestError> {
        let state_changed = self.state.lock().unwrap().request_ime_update(request)?;

        if let Some(allowed) = state_changed {
            let event = WindowEvent::Ime(if allowed { Ime::Enabled } else { Ime::Disabled });
            self.handles.push_window_event(event, self.window_id);
        }

        Ok(())
    }

    #[inline]
    pub(crate) fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        self.state.lock().unwrap().ime_allowed()
    }

    pub(crate) fn focus_window(&self) {}

    pub(crate) fn has_focus(&self) -> bool {
        self.state.lock().unwrap().has_focus()
    }

    pub(crate) fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let state = self.state.lock().unwrap();
        let surface = state.window.wl_surface();
        self.handles.request_user_attention(surface, request_type);
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn set_theme(&self, theme: Option<Theme>) {
        self.state.lock().unwrap().set_theme(theme)
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn theme(&self) -> Option<Theme> {
        self.state.lock().unwrap().theme()
    }

    pub(crate) fn set_content_protected(&self, _protected: bool) {}

    pub(crate) fn set_cursor(&self, cursor: Cursor) {
        let mut state = self.state.lock().unwrap();
        state.set_cursor(cursor);
    }

    pub(crate) fn set_cursor_position(&self, position: Position) -> Result<(), RequestError> {
        self.state
            .lock()
            .unwrap()
            .set_cursor_position(position)
            // Request redraw on success, since the state is double buffered.
            .map(|_| self.request_redraw())
    }

    pub(crate) fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), RequestError> {
        self.state.lock().unwrap().set_cursor_grab(mode)
    }

    pub(crate) fn set_cursor_visible(&self, visible: bool) {
        self.state.lock().unwrap().set_cursor_visible(visible);
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn drag_window(&self) -> Result<(), RequestError> {
        self.state.lock().unwrap().drag_window()
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn drag_resize_window(
        &self,
        direction: ResizeDirection,
    ) -> Result<(), RequestError> {
        self.state.lock().unwrap().drag_resize_window(direction)
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn set_maximized(&self, maximized: bool) {
        let s = self.state.lock().unwrap();
        s.set_maximized(maximized);
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn is_maximized(&self) -> bool {
        let s = self.state.lock().unwrap();
        s.is_maximized()
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn fullscreen(&self) -> Option<Fullscreen> {
        self.state.lock().unwrap().fullscreen()
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        let s = self.state.lock().unwrap();
        s.set_fullscreen(fullscreen)
    }

    /// Only meaningful for a top-level surface.
    pub(super) fn show_window_menu(&self, position: Position) {
        let s = self.state.lock().unwrap();
        let position = position.to_logical(s.scale_factor());
        s.show_window_menu(position);
    }

    pub(crate) fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError> {
        self.handles.set_cursor_hittest(self.state.lock().unwrap().window.wl_surface(), hittest)
    }

    pub(crate) fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        self.state.lock().unwrap().current_monitor()
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
        let raw = rwh_06::WaylandWindowHandle::new({
            let ptr = self.state.lock().unwrap().window.wl_surface().id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_surface will never be null")
        });

        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw.into())) }
    }
}

impl rwh_06::HasDisplayHandle for WindowCommon {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::WaylandDisplayHandle::new({
            let ptr = self.display.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_proxy should never be null")
        });

        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw.into())) }
    }
}
