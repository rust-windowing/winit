use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use dpi::{LogicalPosition, PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use rwh_06::RawWindowHandle;
use sctk::shell::xdg::popup::Popup as SctkPopup;
use sctk::shell::xdg::{XdgPositioner, XdgSurface};
use wayland_client::Proxy;
use wayland_client::protocol::wl_display::WlDisplay;
use wayland_protocols::xdg::shell::client::xdg_positioner::{Anchor, Gravity};
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindow, WindowAttributes, WindowButtons, WindowId,
    WindowLevel,
};

use super::ActiveEventLoop;
use crate::window::WindowRequests;
use crate::window::state::{WindowState, WindowType};

#[derive(Debug)]
pub struct Popup {
    /// Reference to the underlying SCTK popup.
    popup: SctkPopup,

    // The state of the popup.
    popup_state: Arc<Mutex<WindowState>>,

    /// Window id.
    window_id: WindowId,

    /// The wayland display used solely for raw window handle.
    #[allow(dead_code)]
    display: WlDisplay,
}

impl Popup {
    pub(crate) fn new(
        event_loop_window_target: &ActiveEventLoop,
        attributes: WindowAttributes,
    ) -> Result<Self, RequestError> {
        macro_rules! error {
            ($e:literal) => {
                RequestError::NotSupported(NotSupportedError::new($e))
            };
        }

        let parent_window_handle =
            attributes.parent_window().ok_or(error!("Popup without a parent is not supported!"))?;
        if let RawWindowHandle::Wayland(parent_window_handle) = parent_window_handle {
            let queue_handle = event_loop_window_target.queue_handle.clone();
            let mut state = event_loop_window_target.state.borrow_mut();
            let positioner = XdgPositioner::new(&state.xdg_shell)
                .map_err(|_| error!("Failed to create positioner"))?;
            let (popup, popup_state) = if let Some(parent_window_state) = state
                .windows
                .borrow()
                .get(&WindowId::from_raw(parent_window_handle.surface.as_ptr() as usize))
            {
                let size = attributes.surface_size.ok_or(error!("Invalid size for popup"))?;

                let parent_window_state = parent_window_state.lock().unwrap();

                // Use the scale factor and xdg geometry of the parent.
                let scale_factor = parent_window_state.scale_factor();
                let position: LogicalPosition<i32> = attributes
                    .position
                    .ok_or(error!("No position specified"))?
                    .to_logical(scale_factor);
                let geometry_origin = parent_window_state.content_surface_origin();
                // The anchor rect is relative to the parent window geometry, so we need to subtract
                // the geometry origin from the position to get the correct anchor rect.
                let anchor_position = LogicalPosition::new(
                    position.x - geometry_origin.x,
                    position.y - geometry_origin.y,
                );

                positioner.set_anchor(Anchor::TopLeft);
                positioner.set_gravity(Gravity::BottomRight); // Otherwise the child surface will be centered over the anchor point
                positioner.set_anchor_rect(anchor_position.x, anchor_position.y, 1, 1);
                positioner.set_offset(0, 0);
                positioner.set_size(
                    size.to_logical(scale_factor).width,
                    size.to_logical(scale_factor).height,
                );

                let parent_surface = parent_window_state.window.xdg_surface();
                let surface = state.compositor_state.create_surface(&queue_handle);
                let popup = SctkPopup::from_surface(
                    Some(parent_surface),
                    &positioner,
                    &queue_handle,
                    surface.clone(),
                    &state.xdg_shell,
                )
                .map_err(|_| error!("Failed to create popup"))?;
                drop(parent_window_state);

                let popup_state = WindowState::new(
                    event_loop_window_target.handle.clone(),
                    &event_loop_window_target.queue_handle,
                    &state,
                    size,
                    WindowType::Popup((popup.clone(), None)),
                    attributes.preferred_theme,
                    false,
                    scale_factor,
                );

                popup.wl_surface().commit();
                // popup.commit(); Trait not implemented in Sctk

                let popup_state = Arc::new(Mutex::new(popup_state));

                (popup, popup_state)
            } else {
                return Err(error!("Parent window id unknown"));
            };

            let window_id = super::make_wid(&popup.wl_surface());
            state.windows.get_mut().insert(window_id, popup_state.clone());

            let window_requests = WindowRequests {
                redraw_requested: AtomicBool::new(true),
                closed: AtomicBool::new(false),
            };
            let window_requests = Arc::new(window_requests);
            state.window_requests.get_mut().insert(window_id, window_requests.clone());

            let mut wayland_source = event_loop_window_target.wayland_dispatcher.as_source_mut();
            let event_queue = wayland_source.queue();
            // Do a roundtrip.
            event_queue.roundtrip(&mut state).map_err(|err| os_error!(err))?;

            // XXX Wait for the initial configure to arrive.
            while !popup_state.lock().unwrap().is_configured() {
                event_queue.blocking_dispatch(&mut state).map_err(|err| os_error!(err))?;
            }

            Ok(Self {
                popup,
                popup_state,
                window_id,
                display: event_loop_window_target.handle.connection.display().clone(),
            })
        } else {
            Err(RequestError::NotSupported(NotSupportedError::new(
                "Not a wayland window handle passed",
            )))
        }
    }
}

impl CoreWindow for Popup {
    fn id(&self) -> WindowId {
        self.window_id
    }

    fn request_redraw(&self) {
        // // NOTE: try to not wake up the loop when the event was already scheduled and not yet
        // // processed by the loop, because if at this point the value was `true` it could only
        // // mean that the loop still haven't dispatched the value to the client and will do
        // // eventually, resetting it to `false`.
        // if self
        //     .window_requests
        //     .redraw_requested
        //     .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        //     .is_ok()
        // {
        //     self.event_loop_awakener.ping();
        // }
    }

    #[inline]
    fn title(&self) -> String {
        self.popup_state.lock().unwrap().title().to_owned()
    }

    fn pre_present_notify(&self) {
        // self.popup_state.lock().unwrap().request_frame_callback();
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
        let popup_state = self.popup_state.lock().unwrap();
        let scale_factor = popup_state.scale_factor();
        super::logical_to_physical_rounded(popup_state.surface_size(), scale_factor)
    }

    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        // let mut popup_state = self.popup_state.lock().unwrap();
        // let new_size = popup_state.request_surface_size(size);
        // self.request_redraw();
        // Some(new_size)
        None
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        // let popup_state = self.popup_state.lock().unwrap();
        // let scale_factor = popup_state.scale_factor();
        // super::logical_to_physical_rounded(popup_state.outer_size(), scale_factor)
        PhysicalSize::new(100, 100)
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }

    fn set_min_surface_size(&self, min_size: Option<Size>) {
        // let scale_factor = self.scale_factor();
        // let min_size = min_size.map(|size| size.to_logical(scale_factor));
        // self.state.lock().unwrap().set_min_surface_size(min_size);
        // // NOTE: Requires commit to be applied.
        // self.request_redraw();
    }

    /// Set the maximum surface size for the window.
    #[inline]
    fn set_max_surface_size(&self, max_size: Option<Size>) {
        // let scale_factor = self.scale_factor();
        // let max_size = max_size.map(|size| size.to_logical(scale_factor));
        // self.popup_state.lock().unwrap().set_max_surface_size(max_size);
        // // NOTE: Requires commit to be applied.
        // self.request_redraw();
    }

    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        // let popup_state = self.popup_state.lock().unwrap();
        // let scale_factor = popup_state.scale_factor();
        // popup_state
        //     .resize_increments()
        //     .map(|size| super::logical_to_physical_rounded(size, scale_factor))
        None
    }

    fn set_surface_resize_increments(&self, increments: Option<Size>) {
        // let mut popup_state = self.popup_state.lock().unwrap();
        // let scale_factor = popup_state.scale_factor();
        // let increments = increments.map(|size| size.to_logical(scale_factor));
        // popup_state.set_resize_increments(increments);
    }

    fn set_title(&self, title: &str) {
        self.popup_state.lock().unwrap().set_title(title.to_owned());
    }

    #[inline]
    fn set_transparent(&self, transparent: bool) {
        self.popup_state.lock().unwrap().set_transparent(transparent);
    }

    fn set_visible(&self, _visible: bool) {
        // Not possible on Wayland.
    }

    fn is_visible(&self) -> Option<bool> {
        None
    }

    fn set_resizable(&self, resizable: bool) {
        // if self.popup_state.lock().unwrap().set_resizable(resizable) {
        //     // NOTE: Requires commit to be applied.
        //     self.request_redraw();
        // }
    }

    fn is_resizable(&self) -> bool {
        // TODO
        // self.popup_state.lock().unwrap().resizable()
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

    fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        // Not possible for popups
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        None
    }

    #[inline]
    fn scale_factor(&self) -> f64 {
        self.popup_state.lock().unwrap().scale_factor()
    }

    #[inline]
    fn set_blur(&self, blur: bool) {
        // self.popup_state.lock().unwrap().set_blur(blur);
    }

    #[inline]
    fn set_decorations(&self, decorate: bool) {
        // self.popup_state.lock().unwrap().set_decorate(decorate)
    }

    #[inline]
    fn is_decorated(&self) -> bool {
        // self.popup_state.lock().unwrap().is_decorated()
        false
    }

    fn set_window_level(&self, _level: WindowLevel) {}

    fn set_window_icon(&self, window_icon: Option<winit_core::icon::Icon>) {
        // self.popup_state.lock().unwrap().set_window_icon(window_icon)
    }

    #[inline]
    fn request_ime_update(&self, request: ImeRequest) -> Result<(), ImeRequestError> {
        // let state_changed = self.popup_state.lock().unwrap().request_ime_update(request)?;

        // if let Some(allowed) = state_changed {
        //     let event = WindowEvent::Ime(if allowed { Ime::Enabled } else { Ime::Disabled });
        //     self.window_events_sink.lock().unwrap().push_window_event(event, self.window_id);
        //     self.event_loop_awakener.ping();
        // }

        Ok(())
    }

    #[inline]
    fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        // self.popup_state.lock().unwrap().ime_allowed()
        None
    }

    fn focus_window(&self) {}

    fn has_focus(&self) -> bool {
        // self.popup_state.lock().unwrap().has_focus()
        false
    }

    fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        // let xdg_activation = match self.xdg_activation.as_ref() {
        //     Some(xdg_activation) => xdg_activation,
        //     None => {
        //         warn!("`request_user_attention` isn't supported");
        //         return;
        //     },
        // };

        // // Urgency is only removed by the compositor and there's no need to raise urgency when it
        // // was already raised.
        // if request_type.is_none() || self.attention_requested.load(Ordering::Relaxed) {
        //     return;
        // }

        // self.attention_requested.store(true, Ordering::Relaxed);
        // let surface = self.surface().clone();
        // let data = XdgActivationTokenData::Attention((
        //     surface.clone(),
        //     Arc::downgrade(&self.attention_requested),
        // ));
        // let xdg_activation_token = xdg_activation.get_activation_token(&self.queue_handle, data);
        // xdg_activation_token.set_surface(&surface);
        // xdg_activation_token.commit();
    }

    fn set_theme(&self, theme: Option<Theme>) {
        // self.popup_state.lock().unwrap().set_theme(theme)
    }

    fn theme(&self) -> Option<Theme> {
        // self.popup_state.lock().unwrap().theme()
        None
    }

    fn set_content_protected(&self, _protected: bool) {}

    fn set_cursor(&self, cursor: Cursor) {
        // let popup_state = &mut self.popup_state.lock().unwrap();

        // match cursor {
        //     Cursor::Icon(icon) => popup_state.set_cursor(icon),
        //     Cursor::Custom(cursor) => popup_state.set_custom_cursor(cursor),
        // }
    }

    fn set_cursor_position(&self, position: Position) -> Result<(), RequestError> {
        // let scale_factor = self.scale_factor();
        // let position = position.to_logical(scale_factor);
        // self.popup_state
        //     .lock()
        //     .unwrap()
        //     .set_cursor_position(position)
        //     // Request redraw on success, since the state is double buffered.
        //     .map(|_| self.request_redraw())
        Err(RequestError::Ignored)
    }

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), RequestError> {
        // self.popup_state.lock().unwrap().set_cursor_grab(mode)
        Err(RequestError::Ignored)
    }

    fn set_cursor_visible(&self, visible: bool) {
        // self.popup_state.lock().unwrap().set_cursor_visible(visible);
    }

    fn drag_window(&self) -> Result<(), RequestError> {
        // Popup does not support dragging
        Err(RequestError::Ignored)
    }

    fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), RequestError> {
        // TODO: implement
        // self.popup_state.lock().unwrap().drag_resize_window(direction)
        Err(RequestError::Ignored)
    }

    fn show_window_menu(&self, position: Position) {
        // let scale_factor = self.scale_factor();
        // let position = position.to_logical(scale_factor);
        // self.popup_state.lock().unwrap().show_window_menu(position);
    }

    fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError> {
        // let surface = self.window.wl_surface();

        // if hittest {
        //     surface.set_input_region(None);
        //     Ok(())
        // } else {
        //     let region = Region::new(&*self.compositor).map_err(|err| os_error!(err))?;
        //     region.add(0, 0, 0, 0);
        //     surface.set_input_region(Some(region.wl_region()));
        //     Ok(())
        // }
        Err(RequestError::Ignored)
    }

    fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        // let data = self.window.wl_surface().data::<SurfaceData>()?;
        // data.outputs()
        //     .next()
        //     .map(MonitorHandle::new)
        //     .map(|monitor| CoreMonitorHandle(Arc::new(monitor)))
        None
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        // Box::new(
        //     self.monitors
        //         .lock()
        //         .unwrap()
        //         .clone()
        //         .into_iter()
        //         .map(|inner| CoreMonitorHandle(Arc::new(inner))),
        // )
        Box::new([].into_iter())
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

impl rwh_06::HasWindowHandle for Popup {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::WaylandWindowHandle::new({
            let ptr = self.popup.wl_surface().id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_surface will never be null")
        });

        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw.into())) }
    }
}

impl rwh_06::HasDisplayHandle for Popup {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::WaylandDisplayHandle::new({
            let ptr = self.display.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_proxy should never be null")
        });

        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw.into())) }
    }
}
