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
use crate::window::common::WindowCommon;
use crate::window::handles::WindowRequests;
use crate::window::state::{WindowState, WindowType};

#[derive(Debug)]
pub struct Dialog {
    common: WindowCommon,
}

impl Dialog {
    pub(crate) fn new(
        event_loop_window_target: &ActiveEventLoop,
        mut attributes: WindowAttributes,
    ) -> Result<Self, RequestError> {
        fn error(message: &'static str) -> RequestError {
            RequestError::NotSupported(NotSupportedError::new(message))
        }

        let modal = attributes.modal.unwrap_or(false);
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

            let mut dialog_state = WindowState::new(
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

            dialog_state.set_window_icon(attributes.window_icon);

            // Set transparency hint.
            dialog_state.set_transparent(attributes.transparent);

            // Set blur.
            let _ = dialog_state.set_blur(attributes.blur);

            // Set the decorations hint.
            dialog_state.set_decorate(attributes.decorations);

            // Set the window title.
            dialog_state.set_title(attributes.title);

            // Set the min and max sizes. We must set the hints upon creating a window, so
            // we use the default `1.` scaling...
            let min_size = attributes.min_surface_size.map(|size| size.to_logical(1.));
            let max_size = attributes.max_surface_size.map(|size| size.to_logical(1.));
            dialog_state.set_min_surface_size(min_size);
            dialog_state.set_max_surface_size(max_size);

            // Non-resizable implies that the min and max sizes are set to the same value.
            dialog_state.set_resizable(attributes.resizable);

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
        })
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

    fn set_resizable(&self, _resizable: bool) {
        // TODO
        unimplemented!()
    }

    fn is_resizable(&self) -> bool {
        // TODO
        // false
        unimplemented!()
    }

    fn set_enabled_buttons(&self, _buttons: WindowButtons) {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
    }

    fn enabled_buttons(&self) -> WindowButtons {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
        WindowButtons::all()
    }

    fn set_minimized(&self, _minimized: bool) {
        // TODO
        unimplemented!()
    }

    fn is_minimized(&self) -> Option<bool> {
        // XXX clients don't know whether they are minimized or not.
        unimplemented!();
        None
    }

    fn set_maximized(&self, _maximized: bool) {
        // TODO
        unimplemented!()
    }

    fn is_maximized(&self) -> bool {
        // TODO:
        // false
        unimplemented!()
    }

    fn set_fullscreen(&self, _fullscreen: Option<Fullscreen>) {
        // TODO
        unimplemented!()
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        None
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

    fn set_window_level(&self, _level: WindowLevel) {
        // TODO
        unimplemented!()
    }

    fn set_window_icon(&self, _window_icon: Option<winit_core::icon::Icon>) {
        // TODO
        unimplemented!()
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
        // TODO
        unimplemented!()
    }

    fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), RequestError> {
        // TODO
        unimplemented!()
    }

    fn show_window_menu(&self, _position: Position) {
        // TODO
        unimplemented!()
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
