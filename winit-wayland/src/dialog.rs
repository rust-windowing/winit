use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use dpi::{PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use sctk::shell::xdg::window::WindowDecorations;
use winit_core::cursor::Cursor;
use winit_core::error::{CreateWindowError, InvalidInput, NotSupportedError, RequestError};
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindow, WindowAttributes, WindowButtons, WindowId,
    WindowLevel,
};

use super::ActiveEventLoop;
use crate::window::Handles;
use crate::window::common::WindowCommon;
use crate::window::state::WindowType;
use crate::{NonNull, c_void};

#[derive(Debug)]
pub struct Dialog {
    common: WindowCommon,
}

impl Dialog {
    /// Creates a new dialog and if failed because of lack of `xdg_dialog_v1` capability of the
    /// compositor a normal window with a parent is created
    pub(crate) fn create(
        event_loop_window_target: &ActiveEventLoop,
        attributes: WindowAttributes,
    ) -> Result<Box<dyn winit_core::window::Window>, CreateWindowError> {
        let modal = attributes.modal;
        let queue_handle = event_loop_window_target.queue_handle.clone();
        let mut state = event_loop_window_target.state.borrow_mut();
        let monitors = state.monitors.clone();
        let xdg_activation =
            state.xdg_activation.as_ref().map(|activation_state| activation_state.global().clone());

        let Some((_, parent_xdg_toplevel, _)) =
            crate::Window::parent_from_attributes(&mut state, &attributes)?
        else {
            return Err(InvalidInput::new("Dialog without a parent is not supported").into());
        };

        let (dialog_state, window_id, window_requests) = {
            // We prefer server side decorations, however to not have decorations we ask for client
            // side decorations instead.
            let default_decorations = if attributes.decorations {
                WindowDecorations::RequestServer
            } else {
                WindowDecorations::RequestClient
            };

            let surface = state.compositor_state.create_surface(&queue_handle);
            let dialog = match state.xdg_shell.create_dialog(
                surface.clone(),
                default_decorations,
                &queue_handle,
                &parent_xdg_toplevel,
            ) {
                Ok(dialog) => dialog,
                Err(_) => {
                    drop(state); // Window::new() acquires again

                    // Fallback to a normal toplevel window if the dialog is not available
                    return Ok(Box::new(crate::Window::new(event_loop_window_target, attributes)?));
                },
            };

            // Must be before the first commit done in the setup function
            dialog.set_modal(modal);

            let (dialog_state, window_id, window_requests, ..) = crate::Window::setup(
                event_loop_window_target,
                &mut state,
                attributes,
                WindowType::Dialog { dialog: dialog.clone(), last_configure: None },
                &surface,
                "Dialog was dismissed by the compositor before configure",
            )?;

            (dialog_state, window_id, window_requests)
        };

        // Setup the event sync to insert `WindowEvents` right from the window.
        let window_events_sink = state.window_events_sink.clone();
        let event_loop_awakener = event_loop_window_target.event_loop_awakener.clone();

        Ok(Box::new(Self {
            common: WindowCommon {
                state: Arc::downgrade(&dialog_state),
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
            },
        }))
    }

    pub(crate) fn xdg_toplevel(&self) -> Option<NonNull<c_void>> {
        self.common.xdg_toplevel()
    }
}

impl CoreWindow for Dialog {
    fn window_type(&self) -> winit_core::window::WindowType {
        winit_core::window::WindowType::Dialog
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
        // Not possible
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
        // Minimize does not make any sense
        WindowButtons::CLOSE | WindowButtons::MAXIMIZE
    }

    fn set_minimized(&self, _minimized: bool) {
        // A dialog cannot be minimized
    }

    fn is_minimized(&self) -> Option<bool> {
        // XXX clients don't know whether they are minimized or not.
        None
    }

    fn set_maximized(&self, maximized: bool) {
        self.common.set_maximized(maximized);
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
        self.common.is_decorated().unwrap_or_default()
    }

    fn set_window_level(&self, level: WindowLevel) {
        self.common.set_window_level(level)
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
        self.common.show_window_menu(position)
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
