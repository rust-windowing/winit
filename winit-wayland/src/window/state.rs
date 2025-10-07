//! The state of the window, which is shared with the event-loop.

use std::num::NonZeroU32;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use ahash::HashSet;
use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use sctk::compositor::{CompositorState, Region, SurfaceData, SurfaceDataExt};
use sctk::globals::GlobalData;
use sctk::reexports::client::backend::ObjectId;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::protocol::wl_shm::WlShm;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{Proxy, QueueHandle};
use sctk::reexports::csd_frame::{
    DecorationsFrame, FrameAction, FrameClick, ResizeEdge, WindowState as XdgWindowState,
};
use sctk::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1;
use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3;
use sctk::reexports::protocols::wp::viewporter::client::wp_viewport::WpViewport;
use sctk::reexports::protocols::xdg::shell::client::xdg_toplevel::ResizeEdge as XdgResizeEdge;
use sctk::seat::pointer::{PointerDataExt, ThemedPointer};
use sctk::shell::wlr_layer::{LayerSurface, LayerSurfaceConfigure};
use sctk::shell::xdg::window::{DecorationMode, Window, WindowConfigure};
use sctk::shell::xdg::XdgSurface;
use sctk::shell::WaylandSurface;
use sctk::shm::slot::SlotPool;
use sctk::shm::Shm;
use sctk::subcompositor::SubcompositorState;
use tracing::{info, warn};
use wayland_protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1;
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur::OrgKdeKwinBlur;
use winit_core::cursor::{CursorIcon, CustomCursor as CoreCustomCursor};
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme, WindowId,
};

use crate::event_loop::OwnedDisplayHandle;
use crate::logical_to_physical_rounded;
use crate::seat::{
    PointerConstraintsState, TextInputClientState, WinitPointerData, WinitPointerDataExt,
    ZwpTextInputV3Ext,
};
use crate::state::{WindowCompositorUpdate, WinitState};
use crate::types::cursor::{CustomCursor, SelectedCursor, WaylandCustomCursor};
use crate::types::kwin_blur::KWinBlurManager;
use crate::types::xdg_toplevel_icon_manager::ToplevelIcon;

#[cfg(feature = "sctk-adwaita")]
pub type WinitFrame = sctk_adwaita::AdwaitaFrame<WinitState>;
#[cfg(not(feature = "sctk-adwaita"))]
pub type WinitFrame = sctk::shell::xdg::fallback_frame::FallbackFrame<WinitState>;

// Minimum window surface size.
const MIN_WINDOW_SIZE: LogicalSize<u32> = LogicalSize::new(2, 1);

/// The state of the window which is being updated from the [`WinitState`].
#[derive(Debug)]
pub struct WindowState {
    /// The connection to Wayland server.
    pub handle: Arc<OwnedDisplayHandle>,

    /// The `Shm` to set cursor.
    pub shm: WlShm,

    /// The pointers observed on the window.
    pub pointers: Vec<Weak<ThemedPointer<WinitPointerData>>>,

    selected_cursor: SelectedCursor,

    /// Whether the cursor is visible.
    pub cursor_visible: bool,

    /// Pointer constraints to lock/confine pointer.
    pub pointer_constraints: Option<Arc<PointerConstraintsState>>,

    /// Queue handle.
    pub queue_handle: QueueHandle<WinitState>,

    /// State that differs based on being an XDG shell or a WLR layer shell
    pub(self) shell_specific: ShellSpecificState,

    /// Theme variant.
    theme: Option<Theme>,

    /// The current window title.
    title: String,

    /// Xdg toplevel icon manager to request icon setting.
    xdg_toplevel_icon_manager: Option<XdgToplevelIconManagerV1>,

    /// The current window toplevel icon
    toplevel_icon: Option<ToplevelIcon>,

    /// A shared pool where to allocate images (used for window icons and custom cursors)
    image_pool: Arc<Mutex<SlotPool>>,

    // NOTE: we can't use simple counter, since it's racy when seat getting destroyed and new
    // is created, since add/removed stuff could be delivered a bit out of order.
    /// Seats that has keyboard focus on that window.
    seat_focus: HashSet<ObjectId>,

    /// The scale factor of the window.
    scale_factor: f64,

    /// Whether the window is transparent.
    transparent: bool,

    /// The state of the compositor to create WlRegions.
    compositor: Arc<CompositorState>,

    /// The current cursor grabbing mode.
    cursor_grab_mode: GrabState,

    /// The input method properties provided by the application to the IME.
    ///
    /// This state is cached here so that the window can automatically send the state to the IME as
    /// soon as it becomes available without application involvement.
    text_input_state: Option<TextInputClientState>,

    /// The text inputs observed on the window.
    text_inputs: Vec<ZwpTextInputV3>,

    /// The surface size of the window, as in without client side decorations.
    size: LogicalSize<u32>,

    /// Whether we should decorate the frame.
    decorate: bool,

    /// Initial window size provided by the user. Removed on the first
    /// configure.
    initial_size: Option<Size>,

    /// The state of the frame callback.
    frame_callback_state: FrameCallbackState,

    blur: Option<OrgKdeKwinBlur>,
    blur_manager: Option<KWinBlurManager>,

    /// Whether the client side decorations have pending move operations.
    ///
    /// The value is the serial of the event triggered moved.
    has_pending_move: Option<u32>,

    fractional_scale: Option<WpFractionalScaleV1>,
    viewport: Option<WpViewport>,
}

#[derive(Debug)]
enum ShellSpecificState {
    Xdg {
        /// The underlying SCTK window.
        window: Window,

        /// The last received configure.
        last_configure: Option<WindowConfigure>,

        /// Whether the frame is resizable.
        resizable: bool,

        // NOTE: The spec says that destroying parent(`window` in our case), will unmap the
        // subsurfaces. Thus to achieve atomic unmap of the client, drop the decorations
        // frame after the `window` is dropped. To achieve that we rely on rust's struct
        // field drop order guarantees.
        /// The window frame, which is created from the configure request.
        frame: Box<Option<WinitFrame>>,

        /// Whether the CSD fail to create, so we don't try to create them on each iteration.
        csd_fails: bool,

        /// The size of the window when no states were applied to it. The primary use for it
        /// is to fallback to original window size, before it was maximized, if the compositor
        /// sends `None` for the new size in the configure.
        stateless_size: LogicalSize<u32>,

        /// Min size.
        min_surface_size: LogicalSize<u32>,
        max_surface_size: Option<LogicalSize<u32>>,
    },
    WlrLayer {
        surface: LayerSurface,

        last_configure: Option<LayerSurfaceConfigure>,
    },
}

impl WindowState {
    /// Create new window state.
    pub fn new(
        handle: Arc<OwnedDisplayHandle>,
        queue_handle: &QueueHandle<WinitState>,
        winit_state: &WinitState,
        initial_size: Size,
        window: Window,
        theme: Option<Theme>,
    ) -> Self {
        let compositor = winit_state.compositor_state.clone();
        let pointer_constraints = winit_state.pointer_constraints.clone();
        let viewport = winit_state
            .viewporter_state
            .as_ref()
            .map(|state| state.get_viewport(window.wl_surface(), queue_handle));
        let fractional_scale = winit_state
            .fractional_scaling_manager
            .as_ref()
            .map(|fsm| fsm.fractional_scaling(window.wl_surface(), queue_handle));

        let xdg_toplevel_icon_manager = winit_state
            .xdg_toplevel_icon_manager
            .as_ref()
            .map(|toplevel_icon_manager_state| toplevel_icon_manager_state.global().clone());

        Self {
            toplevel_icon: None,
            xdg_toplevel_icon_manager,
            blur: None,
            blur_manager: winit_state.kwin_blur_manager.clone(),
            compositor,
            handle,
            viewport,
            fractional_scale,
            shell_specific: ShellSpecificState::Xdg {
                window,
                last_configure: None,
                resizable: true,
                frame: Box::new(None),
                csd_fails: false,
                stateless_size: initial_size.to_logical(1.),
                max_surface_size: None,
                min_surface_size: MIN_WINDOW_SIZE,
            },
            cursor_grab_mode: GrabState::new(),
            selected_cursor: Default::default(),
            cursor_visible: true,
            decorate: true,
            frame_callback_state: FrameCallbackState::None,
            seat_focus: Default::default(),
            has_pending_move: None,
            text_input_state: None,
            pointer_constraints,
            pointers: Default::default(),
            queue_handle: queue_handle.clone(),
            scale_factor: 1.,
            shm: winit_state.shm.wl_shm().clone(),
            image_pool: winit_state.image_pool.clone(),
            size: initial_size.to_logical(1.),
            initial_size: Some(initial_size),
            text_inputs: Vec::new(),
            theme,
            title: String::default(),
            transparent: false,
        }
    }

    pub fn new_layer(
        handle: Arc<OwnedDisplayHandle>,
        queue_handle: &QueueHandle<WinitState>,
        winit_state: &WinitState,
        initial_size: Size,
        layer_surface: LayerSurface,
        theme: Option<Theme>,
    ) -> Self {
        let compositor = winit_state.compositor_state.clone();
        let pointer_constraints = winit_state.pointer_constraints.clone();

        let fractional_scale = winit_state
            .fractional_scaling_manager
            .as_ref()
            .map(|fsm| fsm.fractional_scaling(layer_surface.wl_surface(), queue_handle));

        let viewport = winit_state
            .viewporter_state
            .as_ref()
            .map(|state| state.get_viewport(layer_surface.wl_surface(), queue_handle));

        Self {
            handle,
            compositor,
            theme,
            cursor_grab_mode: GrabState::new(),
            cursor_visible: true,
            pointer_constraints,
            pointers: Default::default(),
            queue_handle: queue_handle.clone(),
            scale_factor: 1.,
            shm: winit_state.shm.wl_shm().clone(),
            viewport,
            fractional_scale,
            shell_specific: ShellSpecificState::WlrLayer {
                surface: layer_surface,
                last_configure: None,
            },
            size: initial_size.to_logical(1.0),
            selected_cursor: Default::default(),
            decorate: false,
            frame_callback_state: FrameCallbackState::None,
            seat_focus: Default::default(),
            has_pending_move: None,
            initial_size: Some(initial_size),
            text_inputs: Vec::new(),
            title: String::default(),
            transparent: false,
            blur: None,
            blur_manager: winit_state.kwin_blur_manager.clone(),
            toplevel_icon: None,
            xdg_toplevel_icon_manager: winit_state
                .xdg_toplevel_icon_manager
                .as_ref()
                .map(|toplevel_icon_manager_state| toplevel_icon_manager_state.global().clone()),
            image_pool: winit_state.image_pool.clone(),
            text_input_state: None,
        }
    }

    /// Apply closure on the given pointer.
    fn apply_on_pointer<F: FnMut(&ThemedPointer<WinitPointerData>, &WinitPointerData)>(
        &self,
        mut callback: F,
    ) {
        self.pointers.iter().filter_map(Weak::upgrade).for_each(|pointer| {
            let data = pointer.pointer().winit_data();
            callback(pointer.as_ref(), data);
        })
    }

    /// Get the current state of the frame callback.
    pub fn frame_callback_state(&self) -> FrameCallbackState {
        self.frame_callback_state
    }

    /// The frame callback was received, but not yet sent to the user.
    pub fn frame_callback_received(&mut self) {
        self.frame_callback_state = FrameCallbackState::Received;
    }

    /// Reset the frame callbacks state.
    pub fn frame_callback_reset(&mut self) {
        self.frame_callback_state = FrameCallbackState::None;
    }

    /// Request a frame callback if we don't have one for this window in flight.
    pub fn request_frame_callback(&mut self) {
        match &self.frame_callback_state {
            FrameCallbackState::None | FrameCallbackState::Received => {
                self.frame_callback_state = FrameCallbackState::Requested;
                let surface = self.wl_surface();
                surface.frame(&self.queue_handle, surface.clone());
            },
            FrameCallbackState::Requested => (),
        }
    }

    fn wl_surface(&self) -> &WlSurface {
        match &self.shell_specific {
            ShellSpecificState::Xdg { window, .. } => window.wl_surface(),
            ShellSpecificState::WlrLayer { surface, .. } => surface.wl_surface(),
        }
    }

    pub fn configure(
        &mut self,
        configure: WindowConfigure,
        shm: &Shm,
        subcompositor: &Option<Arc<SubcompositorState>>,
    ) -> bool {
        let scale_factor = self.scale_factor();
        let bounds = self.surface_size_bounds(&configure);
        let ShellSpecificState::Xdg {
            ref window,
            ref mut last_configure,
            ref mut frame,
            ref mut csd_fails,
            ref mut stateless_size,
            ..
        } = self.shell_specific
        else {
            unreachable!();
        };
        // NOTE: when using fractional scaling or wl_compositor@v6 the scaling
        // should be delivered before the first configure, thus apply it to
        // properly scale the physical sizes provided by the users.
        if let Some(initial_size) = self.initial_size.take() {
            self.size = initial_size.to_logical(scale_factor);
            *stateless_size = self.size;
        }

        if let Some(subcompositor) = subcompositor.as_ref().filter(|_| {
            configure.decoration_mode == DecorationMode::Client && frame.is_none() && !*csd_fails
        }) {
            match WinitFrame::new(
                window,
                shm,
                #[cfg(feature = "sctk-adwaita")]
                self.compositor.clone(),
                subcompositor.clone(),
                self.queue_handle.clone(),
                #[cfg(feature = "sctk-adwaita")]
                into_sctk_adwaita_config(self.theme),
            ) {
                Ok(mut f) => {
                    f.set_title(&self.title);
                    f.set_scaling_factor(self.scale_factor);
                    // Hide the frame if we were asked to not decorate.
                    f.set_hidden(!self.decorate);
                    **frame = Some(f);
                },
                Err(err) => {
                    warn!("Failed to create client side decorations frame: {err}");
                    *csd_fails = true;
                },
            }
        } else if configure.decoration_mode == DecorationMode::Server {
            // Drop the frame for server side decorations to save resources.
            **frame = None;
        }

        let stateless = Self::is_stateless(&configure);

        let (mut new_size, constrain) = if let Some(frame) = frame.as_mut() {
            // Configure the window states.
            frame.update_state(configure.state);

            match configure.new_size {
                (Some(width), Some(height)) => {
                    let (width, height) = frame.subtract_borders(width, height);
                    let width = width.map(|w| w.get()).unwrap_or(1);
                    let height = height.map(|h| h.get()).unwrap_or(1);
                    ((width, height).into(), false)
                },
                (..) if stateless => (*stateless_size, true),
                _ => (self.size, true),
            }
        } else {
            match configure.new_size {
                (Some(width), Some(height)) => ((width.get(), height.get()).into(), false),
                _ if stateless => (*stateless_size, true),
                _ => (self.size, true),
            }
        };

        // Apply configure bounds only when compositor let the user decide what size to pick.
        if constrain {
            new_size.width =
                bounds.0.map(|bound_w| new_size.width.min(bound_w.get())).unwrap_or(new_size.width);
            new_size.height = bounds
                .1
                .map(|bound_h| new_size.height.min(bound_h.get()))
                .unwrap_or(new_size.height);
        }

        let new_state = configure.state;
        let old_state = last_configure.as_ref().map(|configure| configure.state);

        let state_change_requires_resize = old_state
            .map(|old_state| {
                !old_state
                    .symmetric_difference(new_state)
                    .difference(XdgWindowState::ACTIVATED | XdgWindowState::SUSPENDED)
                    .is_empty()
            })
            // NOTE: `None` is present for the initial configure, thus we must always resize.
            .unwrap_or(true);

        // NOTE: Set the configure before doing a resize, since we query it during it.
        *last_configure = Some(configure);

        if state_change_requires_resize || new_size != self.surface_size() {
            self.resize(new_size);
            true
        } else {
            false
        }
    }

    pub fn configure_layer(&mut self, configure: LayerSurfaceConfigure) {
        let ShellSpecificState::WlrLayer { last_configure, .. } = &mut self.shell_specific else {
            unreachable!();
        };
        // Configure the window states.
        let new_size = match configure.new_size {
            (0, 0) => self.size,
            (0, height) => (self.size.width, height).into(),
            (width, 0) => (width, self.size.height).into(),
            (width, height) => (width, height).into(),
        };

        // XXX Set the configuration before doing a resize.
        *last_configure = Some(configure);

        // XXX Set the configuration before doing a resize.
        self.resize(new_size);
    }

    /// Compute the bounds for the surface size of the surface.
    fn surface_size_bounds(
        &self,
        configure: &WindowConfigure,
    ) -> (Option<NonZeroU32>, Option<NonZeroU32>) {
        let configure_bounds = match configure.suggested_bounds {
            Some((width, height)) => (NonZeroU32::new(width), NonZeroU32::new(height)),
            None => (None, None),
        };

        match &self.shell_specific {
            ShellSpecificState::Xdg { frame, .. } => {
                if let Some(frame) = frame.as_ref() {
                    let (width, height) = frame.subtract_borders(
                        configure_bounds.0.unwrap_or(NonZeroU32::new(1).unwrap()),
                        configure_bounds.1.unwrap_or(NonZeroU32::new(1).unwrap()),
                    );
                    (configure_bounds.0.and(width), configure_bounds.1.and(height))
                } else {
                    configure_bounds
                }
            },
            ShellSpecificState::WlrLayer { surface, .. } => {
                surface.set_size(
                    configure_bounds.0.unwrap_or(NonZeroU32::new(1).unwrap()).into(),
                    configure_bounds.1.unwrap_or(NonZeroU32::new(1).unwrap()).into(),
                );
                configure_bounds
            },
        }
    }

    #[inline]
    fn is_stateless(configure: &WindowConfigure) -> bool {
        !(configure.is_maximized() || configure.is_fullscreen() || configure.is_tiled())
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        match &self.shell_specific {
            ShellSpecificState::Xdg { last_configure, .. } => last_configure
                .as_ref()
                .map(|last_configure| last_configure.is_maximized())
                .unwrap_or_default(),
            ShellSpecificState::WlrLayer { .. } => false,
        }
    }

    #[inline]
    pub fn is_fullscreen(&self) -> bool {
        match &self.shell_specific {
            ShellSpecificState::Xdg { last_configure, .. } => last_configure
                .as_ref()
                .map(|last_configure| last_configure.is_fullscreen())
                .unwrap_or_default(),
            ShellSpecificState::WlrLayer { .. } => false,
        }
    }

    /// Start interacting drag resize.
    pub fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), RequestError> {
        let ShellSpecificState::Xdg { window, .. } = &self.shell_specific else {
            return Ok(());
        };
        let xdg_toplevel = window.xdg_toplevel();

        // TODO(kchibisov) handle touch serials.
        self.apply_on_pointer(|_, data| {
            let serial = data.latest_button_serial();
            let seat = data.seat();
            xdg_toplevel.resize(seat, serial, resize_direction_to_xdg(direction));
        });

        Ok(())
    }

    /// Start the window drag.
    pub fn drag_window(&self) -> Result<(), RequestError> {
        let ShellSpecificState::Xdg { window, .. } = &self.shell_specific else {
            return Ok(());
        };
        let xdg_toplevel = window.xdg_toplevel();

        // TODO(kchibisov) handle touch serials.
        self.apply_on_pointer(|_, data| {
            let serial = data.latest_button_serial();
            let seat = data.seat();
            xdg_toplevel._move(seat, serial);
        });

        Ok(())
    }

    /// Tells whether the window should be closed.
    #[allow(clippy::too_many_arguments)]
    pub fn frame_click(
        &mut self,
        click: FrameClick,
        pressed: bool,
        seat: &WlSeat,
        serial: u32,
        timestamp: Duration,
        window_id: WindowId,
        updates: &mut Vec<WindowCompositorUpdate>,
    ) -> Option<bool> {
        let ShellSpecificState::Xdg { window, frame, .. } = &mut self.shell_specific else {
            return Some(false);
        };
        match (**frame).as_mut()?.on_click(timestamp, click, pressed)? {
            FrameAction::Minimize => window.set_minimized(),
            FrameAction::Maximize => window.set_maximized(),
            FrameAction::UnMaximize => window.unset_maximized(),
            FrameAction::Close => WinitState::queue_close(updates, window_id),
            FrameAction::Move => self.has_pending_move = Some(serial),
            FrameAction::Resize(edge) => {
                let edge = match edge {
                    ResizeEdge::None => XdgResizeEdge::None,
                    ResizeEdge::Top => XdgResizeEdge::Top,
                    ResizeEdge::Bottom => XdgResizeEdge::Bottom,
                    ResizeEdge::Left => XdgResizeEdge::Left,
                    ResizeEdge::TopLeft => XdgResizeEdge::TopLeft,
                    ResizeEdge::BottomLeft => XdgResizeEdge::BottomLeft,
                    ResizeEdge::Right => XdgResizeEdge::Right,
                    ResizeEdge::TopRight => XdgResizeEdge::TopRight,
                    ResizeEdge::BottomRight => XdgResizeEdge::BottomRight,
                    _ => return None,
                };
                window.resize(seat, serial, edge);
            },
            FrameAction::ShowMenu(x, y) => window.show_window_menu(seat, serial, (x, y)),
            _ => (),
        };

        Some(false)
    }

    pub fn frame_point_left(&mut self) {
        let ShellSpecificState::Xdg { ref mut frame, .. } = &mut self.shell_specific else {
            return;
        };
        if let Some(frame) = frame.as_mut() {
            frame.click_point_left();
        }
    }

    // Move the point over decorations.
    pub fn frame_point_moved(
        &mut self,
        seat: &WlSeat,
        surface: &WlSurface,
        timestamp: Duration,
        x: f64,
        y: f64,
    ) -> Option<CursorIcon> {
        let ShellSpecificState::Xdg { window, ref mut frame, .. } = &mut self.shell_specific else {
            return None;
        };

        // Take the serial if we had any, so it doesn't stick around.
        let serial = self.has_pending_move.take();

        if let Some(frame) = frame.as_mut() {
            let cursor = frame.click_point_moved(timestamp, &surface.id(), x, y);
            // If we have a cursor change, that means that cursor is over the decorations,
            // so try to apply move.
            if let Some(serial) = cursor.is_some().then_some(serial).flatten() {
                window.move_(seat, serial);
                None
            } else {
                cursor
            }
        } else {
            None
        }
    }

    /// Get the stored resizable state.
    #[inline]
    pub fn resizable(&self) -> bool {
        match self.shell_specific {
            ShellSpecificState::Xdg { resizable, .. } => resizable,
            ShellSpecificState::WlrLayer { .. } => false,
        }
    }

    /// Set the resizable state on the window.
    ///
    /// Returns `true` when the state was applied.
    #[inline]
    pub fn set_resizable(&mut self, resizable: bool) -> bool {
        match &mut self.shell_specific {
            ShellSpecificState::Xdg { resizable: state_resizable, .. } => {
                if *state_resizable == resizable {
                    return false;
                }

                *state_resizable = resizable;
            },
            ShellSpecificState::WlrLayer { .. } => {
                if resizable {
                    warn!("Resizable is ignored for layer_shell windows");
                }
                return false;
            },
        }

        if resizable {
            // Restore min/max sizes of the window.
            self.reload_min_max_hints();
        } else {
            self.set_min_surface_size(Some(self.size));
            self.set_max_surface_size(Some(self.size));
        }

        // Reload the state on the frame as well.
        match &mut self.shell_specific {
            ShellSpecificState::Xdg { frame, .. } if frame.is_some() => {
                let frame = (**frame).as_mut().unwrap();
                frame.set_resizable(resizable);
                true
            },
            ShellSpecificState::Xdg { .. } => false,
            ShellSpecificState::WlrLayer { .. } => false,
        }
    }

    /// Whether the window is focused by any seat.
    #[inline]
    pub fn has_focus(&self) -> bool {
        !self.seat_focus.is_empty()
    }

    /// Whether the IME is allowed.
    #[inline]
    pub fn ime_allowed(&self) -> Option<ImeCapabilities> {
        self.text_input_state.as_ref().map(|state| state.capabilities())
    }

    pub(crate) fn text_input_state(&self) -> Option<&TextInputClientState> {
        self.text_input_state.as_ref()
    }

    /// Get the size of the window.
    #[inline]
    pub fn surface_size(&self) -> LogicalSize<u32> {
        self.size
    }

    /// Whether the window received initial configure event from the compositor.
    #[inline]
    pub fn is_configured(&self) -> bool {
        match &self.shell_specific {
            ShellSpecificState::Xdg { last_configure, .. } => last_configure.is_some(),
            ShellSpecificState::WlrLayer { last_configure, .. } => last_configure.is_some(),
        }
    }

    #[inline]
    pub fn is_decorated(&mut self) -> bool {
        let ShellSpecificState::Xdg { ref last_configure, ref frame, .. } = self.shell_specific
        else {
            return false;
        };
        let csd = last_configure
            .as_ref()
            .map(|configure| configure.decoration_mode == DecorationMode::Client)
            .unwrap_or(false);
        if let Some(frame) = csd.then_some((**frame).as_ref()).flatten() {
            !frame.is_hidden()
        } else {
            // Server side decorations.
            true
        }
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        Err(NotSupportedError::new("window position information is not available on xdg Wayland")
            .into())
    }

    pub fn set_outer_position(&self, position: Position) {
        let position = position.to_logical(self.scale_factor);

        match &self.shell_specific {
            ShellSpecificState::Xdg { .. } => {
                warn!("Change window position is not available on xdg Wayland",)
            },
            // XXX just works for LayerShell
            // Probably we can save this change to get in the `outer_position` function
            ShellSpecificState::WlrLayer { surface, .. } => {
                surface.set_margin(position.y, 0, 0, position.x)
            },
        }
    }

    /// Get the outer size of the window.
    #[inline]
    pub fn outer_size(&self) -> LogicalSize<u32> {
        match &self.shell_specific {
            ShellSpecificState::Xdg { frame, .. } => (**frame)
                .as_ref()
                .map(|frame| frame.add_borders(self.size.width, self.size.height).into())
                .unwrap_or(self.size),
            ShellSpecificState::WlrLayer { .. } => self.size,
        }
    }

    /// Register pointer on the top-level.
    pub fn pointer_entered(&mut self, added: Weak<ThemedPointer<WinitPointerData>>) {
        self.pointers.push(added);
        self.reload_cursor_style();

        let mode = self.cursor_grab_mode.user_grab_mode;
        let _ = self.set_cursor_grab_inner(mode);
    }

    /// Pointer has left the top-level.
    pub fn pointer_left(&mut self, removed: Weak<ThemedPointer<WinitPointerData>>) {
        let mut new_pointers = Vec::new();
        for pointer in self.pointers.drain(..) {
            if let Some(pointer) = pointer.upgrade() {
                if pointer.pointer() != removed.upgrade().unwrap().pointer() {
                    new_pointers.push(Arc::downgrade(&pointer));
                }
            }
        }

        self.pointers = new_pointers;
    }

    /// Refresh the decorations frame if it's present returning whether the client should redraw.
    pub fn refresh_frame(&mut self) -> bool {
        let ShellSpecificState::Xdg { ref mut frame, .. } = self.shell_specific else {
            return false;
        };
        if let Some(frame) = frame.as_mut() {
            if !frame.is_hidden() && frame.is_dirty() {
                return frame.draw();
            }
        }

        false
    }

    /// Reload the cursor style on the given window.
    pub fn reload_cursor_style(&mut self) {
        if self.cursor_visible {
            match &self.selected_cursor {
                SelectedCursor::Named(icon) => self.set_cursor(*icon),
                SelectedCursor::Custom(cursor) => self.apply_custom_cursor(cursor),
            }
        } else {
            self.set_cursor_visible(self.cursor_visible);
        }
    }

    /// Reissue the transparency hint to the compositor.
    pub fn reload_transparency_hint(&self) {
        let surface = self.wl_surface();

        if self.transparent {
            surface.set_opaque_region(None);
        } else if let Ok(region) = Region::new(&*self.compositor) {
            region.add(0, 0, i32::MAX, i32::MAX);
            surface.set_opaque_region(Some(region.wl_region()));
        } else {
            warn!("Failed to mark window opaque.");
        }
    }

    /// Try to resize the window when the user can do so.
    pub fn request_surface_size(&mut self, surface_size: Size) -> PhysicalSize<u32> {
        if let ShellSpecificState::Xdg { last_configure, .. } = &self.shell_specific {
            if last_configure.as_ref().map(Self::is_stateless).unwrap_or(true) {
                self.resize(surface_size.to_logical(self.scale_factor()))
            }
        };

        logical_to_physical_rounded(self.surface_size(), self.scale_factor())
    }

    /// Resize the window to the new surface size.
    fn resize(&mut self, surface_size: LogicalSize<u32>) {
        self.size = surface_size;

        // Update the stateless size.
        match &mut self.shell_specific {
            ShellSpecificState::Xdg { last_configure, stateless_size, .. } => {
                if Some(true) == last_configure.as_ref().map(Self::is_stateless) {
                    *stateless_size = surface_size;
                }
            },
            ShellSpecificState::WlrLayer { .. } => {},
        }

        // Update the inner frame.
        let ((x, y), outer_size) = match &mut self.shell_specific {
            ShellSpecificState::Xdg { frame, .. } if frame.is_some() => {
                let frame = (**frame).as_mut().unwrap();
                // Resize only visible frame.
                if !frame.is_hidden() {
                    frame.resize(
                        NonZeroU32::new(self.size.width).unwrap(),
                        NonZeroU32::new(self.size.height).unwrap(),
                    );
                }

                (frame.location(), frame.add_borders(self.size.width, self.size.height).into())
            },
            _ => ((0, 0), self.size),
        };

        // Reload the hint.
        self.reload_transparency_hint();

        // Set the window geometry.
        match &self.shell_specific {
            ShellSpecificState::Xdg { window, .. } => {
                window.xdg_surface().set_window_geometry(
                    x,
                    y,
                    outer_size.width as i32,
                    outer_size.height as i32,
                );
            },
            ShellSpecificState::WlrLayer { surface, .. } => {
                surface.set_size(outer_size.width, outer_size.height)
            },
        }

        // Update the target viewport, this is used if and only if fractional scaling is in
        // use.
        if let Some(viewport) = self.viewport.as_ref() {
            // Set inner size without the borders.
            viewport.set_destination(self.size.width as _, self.size.height as _);
        }
    }

    /// Get the scale factor of the window.
    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Set the cursor icon.
    pub fn set_cursor(&mut self, cursor_icon: CursorIcon) {
        self.selected_cursor = SelectedCursor::Named(cursor_icon);

        if !self.cursor_visible {
            return;
        }

        self.apply_on_pointer(|pointer, _| {
            if pointer.set_cursor(&self.handle.connection, cursor_icon).is_err() {
                warn!("Failed to set cursor to {:?}", cursor_icon);
            }
        })
    }

    /// Set the custom cursor icon.
    pub(crate) fn set_custom_cursor(&mut self, cursor: CoreCustomCursor) {
        let cursor = match cursor.cast_ref::<WaylandCustomCursor>() {
            Some(cursor) => cursor,
            None => {
                tracing::error!("unrecognized cursor passed to Wayland backend");
                return;
            },
        };

        let cursor = {
            let mut pool = self.image_pool.lock().unwrap();
            CustomCursor::new(&mut pool, cursor)
        };

        if self.cursor_visible {
            self.apply_custom_cursor(&cursor);
        }

        self.selected_cursor = SelectedCursor::Custom(cursor);
    }

    fn apply_custom_cursor(&self, cursor: &CustomCursor) {
        self.apply_on_pointer(|pointer, data| {
            let surface = pointer.surface();

            let scale = if let Some(viewport) = data.viewport() {
                let scale = self.scale_factor();
                let size = PhysicalSize::new(cursor.w, cursor.h).to_logical(scale);
                viewport.set_destination(size.width, size.height);
                scale
            } else {
                let scale = surface.data::<SurfaceData>().unwrap().surface_data().scale_factor();
                surface.set_buffer_scale(scale);
                scale as f64
            };

            surface.attach(Some(cursor.buffer.wl_buffer()), 0, 0);
            if surface.version() >= 4 {
                surface.damage_buffer(0, 0, cursor.w, cursor.h);
            } else {
                let size = PhysicalSize::new(cursor.w, cursor.h).to_logical(scale);
                surface.damage(0, 0, size.width, size.height);
            }
            surface.commit();

            let serial = pointer
                .pointer()
                .data::<WinitPointerData>()
                .and_then(|data| data.pointer_data().latest_enter_serial())
                .unwrap();

            let hotspot =
                PhysicalPosition::new(cursor.hotspot_x, cursor.hotspot_y).to_logical(scale);
            pointer.pointer().set_cursor(serial, Some(surface), hotspot.x, hotspot.y);
        });
    }

    /// Set maximum inner window size.
    pub fn set_min_surface_size(&mut self, size: Option<LogicalSize<u32>>) {
        match &mut self.shell_specific {
            ShellSpecificState::Xdg { window, frame, min_surface_size, .. } => {
                // Ensure that the window has the right minimum size.
                let mut size = size.unwrap_or(MIN_WINDOW_SIZE);
                size.width = size.width.max(MIN_WINDOW_SIZE.width);
                size.height = size.height.max(MIN_WINDOW_SIZE.height);

                // Add the borders.
                let size = (**frame)
                    .as_ref()
                    .map(|frame| frame.add_borders(size.width, size.height).into())
                    .unwrap_or(size);

                *min_surface_size = size;
                window.set_min_size(Some(size.into()));
            },
            ShellSpecificState::WlrLayer { .. } => {
                warn!("Minimum size is ignored for layer_shell windows")
            },
        }
    }

    /// Set maximum inner window size.
    pub fn set_max_surface_size(&mut self, size: Option<LogicalSize<u32>>) {
        match &mut self.shell_specific {
            ShellSpecificState::Xdg { window, frame, max_surface_size, .. } => {
                let size = size.map(|size| {
                    (**frame)
                        .as_ref()
                        .map(|frame| frame.add_borders(size.width, size.height).into())
                        .unwrap_or(size)
                });

                *max_surface_size = size;
                window.set_max_size(size.map(Into::into));
            },
            ShellSpecificState::WlrLayer { .. } => {
                warn!("Maximum size is ignored for layer_shell windows")
            },
        }
    }

    /// Set the CSD theme.
    pub fn set_theme(&mut self, theme: Option<Theme>) {
        match &mut self.shell_specific {
            ShellSpecificState::Xdg {
                #[cfg(any(
                    feature = "csd-adwaita",
                    feature = "csd-adwaita-crossfont",
                    feature = "csd-adwaita-notitle",
                ))]
                frame,
                ..
            } => {
                self.theme = theme;
                #[cfg(any(
                    feature = "csd-adwaita",
                    feature = "csd-adwaita-crossfont",
                    feature = "csd-adwaita-notitle",
                ))]
                if let Some(frame) = frame.as_mut() {
                    frame.set_config(into_sctk_adwaita_config(theme))
                }
            },
            ShellSpecificState::WlrLayer { .. } => {
                if theme.is_some() {
                    warn!("Theme is ignored for layer_shell windows")
                }
            },
        }
    }

    /// The current theme for CSD decorations.
    #[inline]
    pub fn theme(&self) -> Option<Theme> {
        match &self.shell_specific {
            ShellSpecificState::Xdg { .. } => self.theme,
            ShellSpecificState::WlrLayer { .. } => None,
        }
    }

    /// Set the cursor grabbing state on the top-level.
    pub fn set_cursor_grab(&mut self, mode: CursorGrabMode) -> Result<(), RequestError> {
        if self.cursor_grab_mode.user_grab_mode == mode {
            return Ok(());
        }

        self.set_cursor_grab_inner(mode)?;
        // Update user grab on success.
        self.cursor_grab_mode.user_grab_mode = mode;
        Ok(())
    }

    /// Reload the hints for minimum and maximum sizes.
    pub fn reload_min_max_hints(&mut self) {
        match self.shell_specific {
            ShellSpecificState::Xdg { min_surface_size, max_surface_size, .. } => {
                self.set_min_surface_size(Some(min_surface_size));
                self.set_max_surface_size(max_surface_size);
            },
            ShellSpecificState::WlrLayer { .. } => {},
        }
    }

    /// Set the grabbing state on the surface.
    fn set_cursor_grab_inner(&mut self, mode: CursorGrabMode) -> Result<(), RequestError> {
        let pointer_constraints = match self.pointer_constraints.as_ref() {
            Some(pointer_constraints) => pointer_constraints,
            None if mode == CursorGrabMode::None => return Ok(()),
            None => {
                return Err(
                    NotSupportedError::new("zwp_pointer_constraints is not available").into()
                )
            },
        };

        let mut unset_old = false;
        match self.cursor_grab_mode.current_grab_mode {
            CursorGrabMode::None => unset_old = true,
            CursorGrabMode::Confined => self.apply_on_pointer(|_, data| {
                data.unconfine_pointer();
                unset_old = true;
            }),
            CursorGrabMode::Locked => {
                self.apply_on_pointer(|_, data| {
                    data.unlock_pointer();
                    unset_old = true;
                });
            },
        }

        // In case we haven't unset the old mode, it means that we don't have a cursor above
        // the window, thus just wait for it to re-appear.
        if !unset_old {
            return Ok(());
        }

        let mut set_mode = false;
        let surface = self.wl_surface();
        match mode {
            CursorGrabMode::Locked => self.apply_on_pointer(|pointer, data| {
                let pointer = pointer.pointer();
                data.lock_pointer(pointer_constraints, surface, pointer, &self.queue_handle);
                set_mode = true;
            }),
            CursorGrabMode::Confined => self.apply_on_pointer(|pointer, data| {
                let pointer = pointer.pointer();
                data.confine_pointer(pointer_constraints, surface, pointer, &self.queue_handle);
                set_mode = true;
            }),
            CursorGrabMode::None => {
                // Current lock/confine was already removed.
                set_mode = true;
            },
        }

        // Replace the current grab mode after we've ensure that it got updated.
        if set_mode {
            self.cursor_grab_mode.current_grab_mode = mode;
        }

        Ok(())
    }

    pub fn show_window_menu(&self, position: LogicalPosition<u32>) {
        let ShellSpecificState::Xdg { window, .. } = &self.shell_specific else {
            return;
        };

        // TODO(kchibisov) handle touch serials.
        self.apply_on_pointer(|_, data| {
            let serial = data.latest_button_serial();
            let seat = data.seat();
            window.show_window_menu(seat, serial, position.into());
        });
    }

    /// Set the position of the cursor.
    pub fn set_cursor_position(&self, position: LogicalPosition<f64>) -> Result<(), RequestError> {
        if self.pointer_constraints.is_none() {
            return Err(NotSupportedError::new("zwp_pointer_constraints is not available").into());
        }

        // Position can be set only for locked cursor.
        if self.cursor_grab_mode.current_grab_mode != CursorGrabMode::Locked {
            return Err(NotSupportedError::new(
                "cursor position could only be changed for locked pointer",
            )
            .into());
        }

        self.apply_on_pointer(|_, data| {
            data.set_locked_cursor_position(position.x, position.y);
        });

        Ok(())
    }

    /// Set the visibility state of the cursor.
    pub fn set_cursor_visible(&mut self, cursor_visible: bool) {
        self.cursor_visible = cursor_visible;

        if self.cursor_visible {
            match &self.selected_cursor {
                SelectedCursor::Named(icon) => self.set_cursor(*icon),
                SelectedCursor::Custom(cursor) => self.apply_custom_cursor(cursor),
            }
        } else {
            for pointer in self.pointers.iter().filter_map(|pointer| pointer.upgrade()) {
                let latest_enter_serial = pointer.pointer().winit_data().latest_enter_serial();

                pointer.pointer().set_cursor(latest_enter_serial, None, 0, 0);
            }
        }
    }

    /// Whether show or hide client side decorations.
    #[inline]
    pub fn set_decorate(&mut self, decorate: bool) {
        if decorate == self.decorate {
            return;
        }

        self.decorate = decorate;

        match &mut self.shell_specific {
            ShellSpecificState::Xdg { ref mut frame, window, last_configure, .. } => {
                match last_configure.as_ref().map(|configure| configure.decoration_mode) {
                    Some(DecorationMode::Server) if !self.decorate => {
                        // To disable decorations we should request client and hide the frame.
                        window.request_decoration_mode(Some(DecorationMode::Client))
                    },
                    _ if self.decorate => {
                        window.request_decoration_mode(Some(DecorationMode::Server))
                    },
                    _ => (),
                }

                if let Some(frame) = frame.as_mut() {
                    frame.set_hidden(!decorate);
                    // Force the resize.
                    self.resize(self.size);
                }
            },
            ShellSpecificState::WlrLayer { .. } => {
                if decorate {
                    warn!("Client-side decorations are ignored for layer_shell windows");
                }
            },
        }
    }

    /// Add seat focus for the window.
    #[inline]
    pub fn add_seat_focus(&mut self, seat: ObjectId) {
        self.seat_focus.insert(seat);
    }

    /// Remove seat focus from the window.
    #[inline]
    pub fn remove_seat_focus(&mut self, seat: &ObjectId) {
        self.seat_focus.remove(seat);
    }

    /// Atomically update input method state.
    ///
    /// Returns `None` if an input method state haven't changed. Alternatively `Some(true)` and
    /// `Some(false)` is returned respectfully.
    pub fn request_ime_update(
        &mut self,
        request: ImeRequest,
    ) -> Result<Option<bool>, ImeRequestError> {
        let state_change = match request {
            ImeRequest::Enable(enable) => {
                let (capabilities, request_data) = enable.into_raw();

                if self.text_input_state.is_some() {
                    return Err(ImeRequestError::AlreadyEnabled);
                }

                self.text_input_state = Some(TextInputClientState::new(
                    capabilities,
                    request_data,
                    self.scale_factor(),
                ));
                true
            },
            ImeRequest::Update(request_data) => {
                let scale_factor = self.scale_factor();
                if let Some(text_input_state) = self.text_input_state.as_mut() {
                    text_input_state.update(request_data, scale_factor);
                } else {
                    return Err(ImeRequestError::NotEnabled);
                }
                false
            },
            ImeRequest::Disable => {
                self.text_input_state = None;
                true
            },
        };

        // Only one input method may be active per (seat, surface),
        // but there may be multiple seats focused on a surface,
        // resulting in multiple text input objects.
        //
        // WARNING: this doesn't actually handle different seats with independent cursors. There's
        // no API to set a per-seat input method state, so they all share a single state.
        for text_input in &self.text_inputs {
            text_input.set_state(self.text_input_state.as_ref(), state_change);
        }

        if state_change {
            Ok(Some(self.text_input_state.is_some()))
        } else {
            Ok(None)
        }
    }

    /// Set the scale factor for the given window.
    #[inline]
    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;

        if let ShellSpecificState::Xdg { frame, .. } = &mut self.shell_specific {
            let Some(frame) = (**frame).as_mut() else {
                return;
            };
            frame.set_scaling_factor(scale_factor);

            // NOTE: When fractional scaling is not used update the buffer scale.
            if self.fractional_scale.is_none() {
                self.wl_surface().set_buffer_scale(scale_factor as _);
            }
        }
    }

    /// Make window background blurred
    #[inline]
    pub fn set_blur(&mut self, blurred: bool) {
        let ShellSpecificState::Xdg { window, .. } = &self.shell_specific else {
            return;
        };
        if blurred && self.blur.is_none() {
            if let Some(blur_manager) = self.blur_manager.as_ref() {
                let blur = blur_manager.blur(self.wl_surface(), &self.queue_handle);
                blur.commit();
                self.blur = Some(blur);
            } else {
                info!("Blur manager unavailable, unable to change blur")
            }
        } else if !blurred && self.blur.is_some() {
            self.blur_manager.as_ref().unwrap().unset(window.wl_surface());
            self.blur.take().unwrap().release();
        }
    }

    /// Set the window title to a new value.
    ///
    /// This will automatically truncate the title to something meaningful.
    pub fn set_title(&mut self, mut title: String) {
        // Truncate the title to at most 1024 bytes, so that it does not blow up the protocol
        // messages
        if title.len() > 1024 {
            let mut new_len = 1024;
            while !title.is_char_boundary(new_len) {
                new_len -= 1;
            }
            title.truncate(new_len);
        }

        // Update the CSD title.
        match &mut self.shell_specific {
            ShellSpecificState::Xdg { window, frame, .. } => {
                // Update the CSD title.
                if let Some(frame) = frame.as_mut() {
                    frame.set_title(&title);
                }
                window.set_title(&title);
            },
            ShellSpecificState::WlrLayer { .. } => {},
        }
        self.title = title;
    }

    /// Set the window's icon
    pub fn set_window_icon(&mut self, window_icon: Option<winit_core::icon::Icon>) {
        let xdg_toplevel = match &mut self.shell_specific {
            ShellSpecificState::Xdg { window, .. } => window.xdg_toplevel(),
            ShellSpecificState::WlrLayer { .. } => {
                warn!("xdg_toplevel is not supported by layer_shell");
                return;
            },
        };
        let xdg_toplevel_icon_manager = match self.xdg_toplevel_icon_manager.as_ref() {
            Some(xdg_toplevel_icon_manager) => xdg_toplevel_icon_manager,
            None => {
                warn!("`xdg_toplevel_icon_manager_v1` is not supported");
                return;
            },
        };

        let (toplevel_icon, xdg_toplevel_icon) = match window_icon {
            Some(icon) => {
                let mut image_pool = self.image_pool.lock().unwrap();
                let toplevel_icon = match ToplevelIcon::new(icon, &mut image_pool) {
                    Ok(toplevel_icon) => toplevel_icon,
                    Err(error) => {
                        warn!("Error setting window icon: {error}");
                        return;
                    },
                };

                let xdg_toplevel_icon =
                    xdg_toplevel_icon_manager.create_icon(&self.queue_handle, GlobalData);

                toplevel_icon.add_buffer(&xdg_toplevel_icon);

                (Some(toplevel_icon), Some(xdg_toplevel_icon))
            },
            None => (None, None),
        };

        xdg_toplevel_icon_manager.set_icon(xdg_toplevel, xdg_toplevel_icon.as_ref());
        self.toplevel_icon = toplevel_icon;

        if let Some(xdg_toplevel_icon) = xdg_toplevel_icon {
            xdg_toplevel_icon.destroy();
        }
    }

    /// Mark the window as transparent.
    #[inline]
    pub fn set_transparent(&mut self, transparent: bool) {
        self.transparent = transparent;
        self.reload_transparency_hint();
    }

    /// Register text input on the top-level.
    #[inline]
    pub fn text_input_entered(&mut self, text_input: &ZwpTextInputV3) {
        if !self.text_inputs.iter().any(|t| t == text_input) {
            self.text_inputs.push(text_input.clone());
        }
    }

    /// The text input left the top-level.
    #[inline]
    pub fn text_input_left(&mut self, text_input: &ZwpTextInputV3) {
        if let Some(position) = self.text_inputs.iter().position(|t| t == text_input) {
            self.text_inputs.remove(position);
        }
    }

    /// Get the cached title.
    #[inline]
    pub fn title(&self) -> &str {
        &self.title
    }
}

impl Drop for WindowState {
    fn drop(&mut self) {
        if let Some(blur) = self.blur.take() {
            blur.release();
        }

        if let Some(fs) = self.fractional_scale.take() {
            fs.destroy();
        }

        if let Some(viewport) = self.viewport.take() {
            viewport.destroy();
        }

        // NOTE: the wl_surface used by the window is being cleaned up when
        // dropping SCTK `Window`.
    }
}

/// The state of the cursor grabs.
#[derive(Clone, Copy, Debug)]
struct GrabState {
    /// The grab mode requested by the user.
    user_grab_mode: CursorGrabMode,

    /// The current grab mode.
    current_grab_mode: CursorGrabMode,
}

impl GrabState {
    fn new() -> Self {
        Self { user_grab_mode: CursorGrabMode::None, current_grab_mode: CursorGrabMode::None }
    }
}

/// The state of the frame callback.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameCallbackState {
    /// No frame callback was requested.
    #[default]
    None,
    /// The frame callback was requested, but not yet arrived, the redraw events are throttled.
    Requested,
    /// The callback was marked as done, and user could receive redraw requested
    Received,
}

fn resize_direction_to_xdg(direction: ResizeDirection) -> XdgResizeEdge {
    match direction {
        ResizeDirection::North => XdgResizeEdge::Top,
        ResizeDirection::West => XdgResizeEdge::Left,
        ResizeDirection::NorthWest => XdgResizeEdge::TopLeft,
        ResizeDirection::NorthEast => XdgResizeEdge::TopRight,
        ResizeDirection::East => XdgResizeEdge::Right,
        ResizeDirection::SouthWest => XdgResizeEdge::BottomLeft,
        ResizeDirection::SouthEast => XdgResizeEdge::BottomRight,
        ResizeDirection::South => XdgResizeEdge::Bottom,
    }
}

// NOTE: Rust doesn't allow `From<Option<Theme>>`.
#[cfg(feature = "sctk-adwaita")]
fn into_sctk_adwaita_config(theme: Option<Theme>) -> sctk_adwaita::FrameConfig {
    match theme {
        Some(Theme::Light) => sctk_adwaita::FrameConfig::light(),
        Some(Theme::Dark) => sctk_adwaita::FrameConfig::dark(),
        None => sctk_adwaita::FrameConfig::auto(),
    }
}
