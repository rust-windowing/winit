use core::sync::atomic::Ordering;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, Weak};

use dpi::{
    LogicalPosition, LogicalSize, PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size,
};
use rwh_06::RawWindowHandle;
use sctk::compositor::SurfaceData;
use sctk::shell::WaylandSurface;
use sctk::shell::xdg::popup::Popup as SctkPopup;
use sctk::shell::xdg::{XdgPositioner, XdgSurface};
use wayland_client::Proxy;
use wayland_client::protocol::wl_display::WlDisplay;
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event::{Ime, WindowEvent};
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindow, WindowAnchor, WindowAttributes, WindowButtons,
    WindowConstraintAdjustment, WindowGravity, WindowId, WindowLevel, WindowPositioner,
};

use super::ActiveEventLoop;
use super::output::MonitorHandle;
use crate::WindowAttributesWayland;
use crate::window::Handles;
use crate::window::common::WindowCommon;
use crate::window::handles::WindowRequests;
use crate::window::state::{WindowState, WindowType};

#[derive(Debug)]
pub struct Popup {
    common: WindowCommon,
}

impl Popup {
    pub(crate) fn new(
        event_loop_window_target: &ActiveEventLoop,
        mut attributes: WindowAttributes,
    ) -> Result<Self, RequestError> {
        fn error(message: &'static str) -> RequestError {
            RequestError::NotSupported(NotSupportedError::new(message))
        }

        let parent_window_handle =
            attributes.parent_window().ok_or(error("Popup without a parent is not supported!"))?;
        if let RawWindowHandle::Wayland(parent_window_handle) = parent_window_handle {
            let queue_handle = event_loop_window_target.queue_handle.clone();
            let mut state = event_loop_window_target.state.borrow_mut();
            let monitors = state.monitors.clone();
            let xdg_activation = state
                .xdg_activation
                .as_ref()
                .map(|activation_state| activation_state.global().clone());
            let xdg_positioner = XdgPositioner::new(&state.xdg_shell)
                .map_err(|_| error("Failed to create positioner"))?;
            let parent_window_id =
                WindowId::from_raw(parent_window_handle.surface.as_ptr() as usize);
            let (popup, popup_state) = if let Some(parent_window_state) =
                state.windows.borrow().get(&parent_window_id)
            {
                let WindowPositioner {
                    anchor,
                    anchor_rect,
                    offset: positioner_offset,
                    gravity,
                    constraint_adjustment,
                    ..
                } = attributes.positioner.unwrap_or_default();
                let grab_keyboard = attributes.active;

                let mut parent_window_state = parent_window_state.lock().unwrap();

                // Use the scale factor and xdg geometry of the parent.
                let scale_factor = parent_window_state.scale_factor();
                let size = attributes
                    .surface_size
                    .ok_or(error("Invalid size for popup"))?
                    .to_logical(scale_factor);
                if size.width == 0_i32 || size.height == 0_i32 {
                    return Err(error("The popups size must not be zero"));
                }

                // Anchoring
                // The anchor rect is relative to the parent window geometry, so we need to subtract
                // the geometry origin from the position to get the correct anchor rect.
                // This is important for client side decorations
                let geometry_origin = parent_window_state.content_surface_origin();
                let anchor_position = LogicalPosition::new(-geometry_origin.x, -geometry_origin.y);
                if xdg_positioner.version() >= 3 {
                    xdg_positioner.set_reactive();
                }
                xdg_positioner.set_anchor(from_anchor(anchor));
                xdg_positioner.set_gravity(from_gravity(gravity));
                xdg_positioner
                    .set_constraint_adjustment(from_constraint_adjustment(constraint_adjustment));

                let (anchor_rect_position, anchor_rect_size) = if attributes.positioner.is_some() {
                    let size = anchor_rect.1.to_logical::<i32>(scale_factor);
                    (
                        anchor_rect.0.to_logical::<i32>(scale_factor),
                        LogicalSize::new(size.width.max(1), size.height.max(1)),
                    )
                } else {
                    // anchor rect was not specified use attributes.position
                    let pos: LogicalPosition<i32> = attributes
                        .position
                        .map(|position| position.to_logical(scale_factor))
                        .unwrap_or_default();
                    (pos, LogicalSize::new(1, 1))
                };

                let anchor_rect = (
                    LogicalPosition::new(
                        anchor_rect_position.x + anchor_position.x,
                        anchor_rect_position.y + anchor_position.y,
                    ),
                    anchor_rect_size,
                );
                xdg_positioner.set_anchor_rect(
                    anchor_rect.0.x,
                    anchor_rect.0.y,
                    anchor_rect.1.width,
                    anchor_rect.1.height,
                );
                let offset: LogicalPosition<i32> = positioner_offset.to_logical(scale_factor);
                xdg_positioner.set_offset(offset.x, offset.y);
                xdg_positioner.set_size(size.width, size.height);

                let parent_surface = parent_window_state.window.xdg_surface();
                let surface = state.compositor_state.create_surface(&queue_handle);
                let popup = SctkPopup::from_surface(
                    Some(parent_surface),
                    &xdg_positioner,
                    &queue_handle,
                    surface.clone(),
                    &state.xdg_shell,
                )
                .map_err(|_| error("Failed to create popup"))?;
                parent_window_state.add_child(super::make_wid(popup.wl_surface()));
                drop(parent_window_state);

                let mut popup_state = WindowState::new(
                    event_loop_window_target,
                    &state,
                    size.into(),
                    WindowType::Popup {
                        popup: popup.clone(),
                        xdg_positioner,
                        last_configure: None,
                        parent_origin: geometry_origin,
                        positioner: WindowPositioner::new(
                            anchor,
                            (anchor_rect_position.into(), anchor_rect_size.into()),
                            positioner_offset,
                            gravity,
                            constraint_adjustment,
                        ),
                    },
                    attributes.preferred_theme,
                    false,
                    scale_factor,
                    Some(parent_window_id),
                );

                // Set transparency hint.
                popup_state.set_transparent(attributes.transparent);

                // Set blur.
                let _ = popup_state.set_blur(attributes.blur);

                let WindowAttributesWayland { activation_token, .. } = *attributes
                    .platform
                    .take()
                    .and_then(|p| p.cast::<WindowAttributesWayland>().ok())
                    .unwrap_or_default();

                // Activate the window when the token is passed.
                if let (Some(xdg_activation), Some(token)) =
                    (xdg_activation.as_ref(), activation_token)
                {
                    xdg_activation.activate(token.into_raw(), &surface);
                }

                // Request a keyboard grab so the compositor routes key events to
                // this popup rather than the parent window. Must happen before the
                // first commit that maps the surface.
                if grab_keyboard {
                    // Use the seat with the most recent event
                    let grab = state
                        .seat_state
                        .seats()
                        .filter_map(|seat| {
                            let serial = state.seats.get(&seat.id())?.latest_serial()?;
                            Some((seat, serial))
                        })
                        .max_by_key(|(_, serial)| *serial);

                    if let Some((seat, serial)) = grab {
                        popup.xdg_popup().grab(&seat, serial);
                    }
                }

                // Do initial commit
                popup.commit();

                let popup_state = Arc::new(Mutex::new(popup_state));

                (popup, popup_state)
            } else {
                return Err(error("Parent window id unknown"));
            };

            let window_id = super::make_wid(popup.wl_surface());
            state.windows.get_mut().insert(window_id, popup_state.clone());

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
            while !popup_state.lock().unwrap().is_configured() {
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
                    state: Arc::downgrade(&popup_state),
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
        } else {
            Err(RequestError::NotSupported(NotSupportedError::new(
                "A Popup requires a parent wayland window handle",
            )))
        }
    }
}

impl CoreWindow for Popup {
    fn window_type(&self) -> winit_core::window::WindowType {
        winit_core::window::WindowType::Popup
    }

    fn positioner(&self) -> WindowPositioner {
        let Some(state) = self.common.state.upgrade() else { return WindowPositioner::default() };
        if let WindowType::Popup { positioner, .. } = &state.lock().unwrap().window {
            *positioner
        } else {
            WindowPositioner::default()
        }
    }

    fn set_positioner(&self, new_positioner: WindowPositioner) {
        let Some(state) = self.common.state.upgrade() else {
            return;
        };

        let mut state = state.lock().unwrap();
        let scale_factor = state.scale_factor();

        if let WindowType::Popup { popup, xdg_positioner, parent_origin, positioner, .. } =
            &mut state.window
        {
            *positioner = new_positioner;

            xdg_positioner.set_anchor(from_anchor(new_positioner.anchor));
            xdg_positioner.set_gravity(from_gravity(new_positioner.gravity));
            xdg_positioner.set_constraint_adjustment(from_constraint_adjustment(
                new_positioner.constraint_adjustment,
            ));

            let (position, size) = new_positioner.anchor_rect;
            let size: LogicalSize<i32> = size.to_logical(scale_factor);
            let position: LogicalPosition<i32> = position.to_logical(scale_factor);
            xdg_positioner.set_anchor_rect(
                position.x - parent_origin.x,
                position.y - parent_origin.y,
                size.width.max(1),
                size.height.max(1),
            );

            let offset: LogicalPosition<i32> = new_positioner.offset.to_logical(scale_factor);
            xdg_positioner.set_offset(offset.x, offset.y);

            if popup.xdg_popup().version() >= 3 {
                popup.reposition(xdg_positioner, 0);
            }
        }
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
        let s = self
            .common
            .state
            .upgrade()
            .ok_or_else(|| NotSupportedError::new("the popup has been destroyed"))?;
        let state = s.lock().unwrap();
        if let WindowType::Popup { last_configure: Some(configure), .. } = &state.window {
            let (x, y) = configure.position;
            return Ok(LogicalPosition::new(x, y).to_physical(state.scale_factor()));
        }
        Err(NotSupportedError::new("the popup has not been configured yet").into())
    }

    fn set_outer_position(&self, position: Position) {
        let Some(s) = self.common.state.upgrade() else { return };
        let mut state = s.lock().unwrap();
        let scale_factor = state.scale_factor();
        if let WindowType::Popup { popup, xdg_positioner, positioner, parent_origin, .. } =
            &mut state.window
        {
            let size = positioner.anchor_rect.1;
            positioner.anchor_rect = (position, size);

            let logical_position: LogicalPosition<i32> = position.to_logical(scale_factor);
            let logical_size: LogicalSize<i32> = size.to_logical(scale_factor);
            xdg_positioner.set_anchor_rect(
                logical_position.x - parent_origin.x,
                logical_position.y - parent_origin.y,
                logical_size.width,
                logical_size.height,
            );
            if popup.xdg_popup().version() >= 3 {
                popup.reposition(xdg_positioner, 0);
            }
        }
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
        self.common.scale_factor()
    }

    #[inline]
    fn set_blur(&self, blur: bool) {
        self.common.set_blur(blur);
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
        self.common.request_user_attention(request_type)
    }

    fn set_theme(&self, _theme: Option<Theme>) {
        // A popup does not have a frame
    }

    fn theme(&self) -> Option<Theme> {
        // A popup does not have a frame
        None
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

fn from_gravity(
    gravity: WindowGravity,
) -> wayland_protocols::xdg::shell::client::xdg_positioner::Gravity {
    use wayland_protocols::xdg::shell::client::xdg_positioner::Gravity;
    match gravity {
        WindowGravity::Center => Gravity::None,
        WindowGravity::Top => Gravity::Top,
        WindowGravity::Bottom => Gravity::Bottom,
        WindowGravity::Left => Gravity::Left,
        WindowGravity::Right => Gravity::Right,
        WindowGravity::TopLeft => Gravity::TopLeft,
        WindowGravity::BottomLeft => Gravity::BottomLeft,
        WindowGravity::TopRight => Gravity::TopRight,
        WindowGravity::BottomRight => Gravity::BottomRight,
        _ => Gravity::None,
    }
}

fn from_anchor(
    value: WindowAnchor,
) -> wayland_protocols::xdg::shell::client::xdg_positioner::Anchor {
    use wayland_protocols::xdg::shell::client::xdg_positioner::Anchor;
    match value {
        WindowAnchor::Center => Anchor::None,
        WindowAnchor::Top => Anchor::Top,
        WindowAnchor::Bottom => Anchor::Bottom,
        WindowAnchor::Left => Anchor::Left,
        WindowAnchor::Right => Anchor::Right,
        WindowAnchor::TopLeft => Anchor::TopLeft,
        WindowAnchor::BottomLeft => Anchor::BottomLeft,
        WindowAnchor::TopRight => Anchor::TopRight,
        WindowAnchor::BottomRight => Anchor::BottomRight,
        _ => Anchor::None,
    }
}

fn from_constraint_adjustment(
    value: WindowConstraintAdjustment,
) -> wayland_protocols::xdg::shell::client::xdg_positioner::ConstraintAdjustment {
    use wayland_protocols::xdg::shell::client::xdg_positioner::ConstraintAdjustment;

    const _: () = {
        assert!(WindowConstraintAdjustment::SLIDE_X.bits() == ConstraintAdjustment::SlideX.bits());
        assert!(WindowConstraintAdjustment::SLIDE_Y.bits() == ConstraintAdjustment::SlideY.bits());
        assert!(WindowConstraintAdjustment::FLIP_X.bits() == ConstraintAdjustment::FlipX.bits());
        assert!(WindowConstraintAdjustment::FLIP_Y.bits() == ConstraintAdjustment::FlipY.bits());
        assert!(
            WindowConstraintAdjustment::RESIZE_X.bits() == ConstraintAdjustment::ResizeX.bits()
        );
        assert!(
            WindowConstraintAdjustment::RESIZE_Y.bits() == ConstraintAdjustment::ResizeY.bits()
        );
    };

    ConstraintAdjustment::from_bits_retain(value.bits())
}
