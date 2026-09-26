//! The state of the window, which is shared with the event-loop.

use std::num::NonZeroU32;
use std::sync::{Arc, Mutex, Weak};

use dpi::{LogicalPosition, LogicalSize, PhysicalSize, Size};
use foldhash::HashSet;
use sctk::compositor::{CompositorState, Region, SurfaceData};
use sctk::globals::GlobalData;
use sctk::reexports::client::backend::ObjectId;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::protocol::wl_shm::WlShm;
use sctk::reexports::client::{Proxy, QueueHandle};
use sctk::reexports::csd_frame::DecorationsFrame;
use sctk::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1;
use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3;
use sctk::reexports::protocols::wp::viewporter::client::wp_viewport::WpViewport;
use sctk::reexports::protocols::xdg::shell::client::xdg_toplevel::ResizeEdge as XdgResizeEdge;
use sctk::seat::pointer::{PointerData, ThemedPointer};
use sctk::shell::WaylandSurface;
use sctk::shell::xdg::XdgSurface;
use sctk::shell::xdg::window::WindowConfigure;
use sctk::shm::slot::SlotPool;
use tracing::{info, warn};
use wayland_protocols::xdg::shell::client::xdg_toplevel;
use wayland_protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{ResizeDirection, Theme, WindowId};

use crate::event_loop::OwnedDisplayHandle;
use crate::seat::{
    PointerConstraintsState, TextInputClientState, WinitPointerData, WinitPointerDataExt,
};
use crate::state::WinitState;
use crate::types::bgr_effects::{BgrEffectManager, SurfaceBlurEffect};
use crate::types::cursor::SelectedCursor;
use crate::types::xdg_toplevel_icon_manager::ToplevelIcon;
use crate::{ActiveEventLoop, logical_to_physical_rounded, output};

mod configure;
mod cursor;
mod frame;
mod ime;
mod window_type;
use cursor::GrabState;
pub use frame::FrameCallbackState;
pub use window_type::WindowType;

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

    /// The seat and serial of the touch currently down on the window, if any.
    pub touch_down: Option<(WlSeat, u32)>,

    selected_cursor: SelectedCursor,

    /// Whether the cursor is visible.
    pub cursor_visible: bool,

    /// Pointer constraints to lock/confine pointer.
    pub pointer_constraints: Option<Arc<PointerConstraintsState>>,

    /// Queue handle.
    pub queue_handle: QueueHandle<WinitState>,

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

    /// Whether the frame is resizable.
    resizable: bool,

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

    /// Whether the CSD fail to create, so we don't try to create them on each iteration.
    csd_fails: bool,

    /// Whether we should decorate the frame.
    decorate: bool,

    /// Whether we should tell the compositor that we prefer drawing decorations ourself.
    prefer_csd: bool,

    /// Min size.
    min_surface_size: LogicalSize<u32>,
    max_surface_size: Option<LogicalSize<u32>>,
    resize_increments: Option<LogicalSize<u32>>,

    /// The size of the window when no states were applied to it. The primary use for it
    /// is to fallback to original window size, before it was maximized, if the compositor
    /// sends `None` for the new size in the configure.
    stateless_size: LogicalSize<u32>,

    /// Initial window size provided by the user. Removed on the first
    /// configure.
    initial_size: Option<Size>,

    /// The state of the frame callback.
    frame_callback_state: FrameCallbackState,

    viewport: Option<WpViewport>,
    fractional_scale: Option<WpFractionalScaleV1>,
    blur: Option<SurfaceBlurEffect>,
    blur_manager: Option<BgrEffectManager>,

    /// Whether the client side decorations have pending move operations.
    ///
    /// The value is the serial of the event triggered moved.
    has_pending_move: Option<u32>,

    /// The underlying SCTK window.
    pub window: WindowType,

    // NOTE: The spec says that destroying parent(`window` in our case), will unmap the
    // subsurfaces. Thus to achieve atomic unmap of the client, drop the decorations
    // frame after the `window` is dropped. To achieve that we rely on rust's struct
    // field drop order guarantees.
    /// The window frame, which is created from the configure request.
    frame: Option<WinitFrame>,

    /// Parent Window if available
    parent: Option<WindowId>,

    /// Children of this window like popups, dialogs or other windows
    children: Vec<WindowId>,
}

impl WindowState {
    /// Create new window state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        active_event_loop: &ActiveEventLoop,
        winit_state: &WinitState,
        initial_size: Size,
        window: WindowType,
        theme: Option<Theme>,
        prefer_csd: bool,
        scale_factor: f64,
        parent: Option<WindowId>,
    ) -> Self {
        let handle = active_event_loop.handle.clone();
        let queue_handle = &active_event_loop.queue_handle;
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
            blur_manager: winit_state.blur_manager.clone(),
            compositor,
            handle,
            csd_fails: false,
            cursor_grab_mode: GrabState::new(),
            selected_cursor: Default::default(),
            cursor_visible: true,
            decorate: true,
            prefer_csd,
            fractional_scale,
            frame: None,
            frame_callback_state: FrameCallbackState::None,
            seat_focus: Default::default(),
            has_pending_move: None,
            text_input_state: None,
            max_surface_size: None,
            min_surface_size: LogicalSize::new(0, 0),
            resize_increments: None,
            pointer_constraints,
            pointers: Default::default(),
            touch_down: None,
            queue_handle: queue_handle.clone(),
            resizable: true,
            scale_factor,
            shm: winit_state.shm.wl_shm().clone(),
            image_pool: winit_state.image_pool.clone(),
            size: initial_size.to_logical(1.),
            stateless_size: initial_size.to_logical(1.),
            initial_size: Some(initial_size),
            text_inputs: Vec::new(),
            theme,
            title: String::default(),
            transparent: false,
            viewport,
            window,
            children: Default::default(),
            parent,
        }
    }

    pub(crate) fn xdg_toplevel(&self) -> Option<&xdg_toplevel::XdgToplevel> {
        self.window.xdg_toplevel()
    }

    // HACK: Currently to get the data device to initiate a drag-and-drop, we iterate through all
    // focused seats to find one with a pointer capability. This is definitely wrong.
    pub(crate) fn focused_seats(&self) -> impl Iterator<Item = &ObjectId> {
        self.seat_focus.iter()
    }

    /// Apply closure on the given pointer.
    fn apply_on_pointer<
        F: FnMut(&ThemedPointer<WinitPointerData>, &PointerData<WinitPointerData>),
    >(
        &self,
        mut callback: F,
    ) {
        self.pointers.iter().filter_map(Weak::upgrade).for_each(|pointer| {
            let data = pointer.pointer().winit_data();
            callback(pointer.as_ref(), data);
        })
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

        if let Some(frame) = self.frame.as_ref() {
            let (width, height) = frame.subtract_borders(
                configure_bounds.0.unwrap_or(NonZeroU32::new(1).unwrap()),
                configure_bounds.1.unwrap_or(NonZeroU32::new(1).unwrap()),
            );
            (configure_bounds.0.and(width), configure_bounds.1.and(height))
        } else {
            configure_bounds
        }
    }

    #[inline]
    fn is_stateless(configure: &WindowConfigure) -> bool {
        !(configure.is_maximized() || configure.is_fullscreen() || configure.is_tiled())
    }

    /// Start interacting drag resize.
    pub fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), RequestError> {
        let xdg_toplevel = match &self.window {
            WindowType::Window { window, .. } => window.xdg_toplevel(),
            WindowType::Popup { .. } => {
                return Err(RequestError::NotSupported(NotSupportedError::new(
                    "drag_resize_window is not supported for WindowType::Popup",
                )));
            },
        };

        if let Some((seat, serial)) = &self.touch_down {
            xdg_toplevel.resize(seat, *serial, resize_direction_to_xdg(direction));
            return Ok(());
        }

        self.apply_on_pointer(|_, data| {
            if let Some(serial) = data.latest_button_serial() {
                let seat = data.seat();
                xdg_toplevel.resize(seat, serial, resize_direction_to_xdg(direction));
            }
        });
        Ok(())
    }

    /// Start the window drag.
    pub fn drag_window(&self) -> Result<(), RequestError> {
        let xdg_toplevel = match &self.window {
            WindowType::Window { window, .. } => window.xdg_toplevel(),
            WindowType::Popup { .. } => {
                return Err(RequestError::NotSupported(NotSupportedError::new(
                    "drag_window is not supported for WindowType::Popup",
                )));
            },
        };

        if let Some((seat, serial)) = &self.touch_down {
            xdg_toplevel._move(seat, *serial);
            return Ok(());
        }

        self.apply_on_pointer(|_, data| {
            if let Some(serial) = data.latest_button_serial() {
                let seat = data.seat();
                xdg_toplevel._move(seat, serial);
            }
        });

        Ok(())
    }

    /// Get the stored resizable state.
    #[inline]
    pub fn resizable(&self) -> bool {
        self.resizable
    }

    /// Set the resizable state on the window.
    ///
    /// Returns `true` when the state was applied.
    #[inline]
    pub fn set_resizable(&mut self, resizable: bool) -> bool {
        if self.resizable == resizable {
            return false;
        }

        self.resizable = resizable;
        self.reload_min_max_hints();

        // Reload the state on the frame as well.
        if let Some(frame) = self.frame.as_mut() {
            frame.set_resizable(resizable);
        }

        true
    }

    /// Whether the window is focused by any seat.
    #[inline]
    pub fn has_focus(&self) -> bool {
        !self.seat_focus.is_empty()
    }

    /// Get the size of the window.
    #[inline]
    pub fn surface_size(&self) -> LogicalSize<u32> {
        self.size
    }

    /// Whether the window received initial configure event from the compositor.
    #[inline]
    pub fn is_configured(&self) -> bool {
        self.window.is_configured()
    }

    /// Get the outer size of the window.
    #[inline]
    pub fn outer_size(&self) -> LogicalSize<u32> {
        self.frame
            .as_ref()
            .map(|frame| frame.add_borders(self.size.width, self.size.height).into())
            .unwrap_or(self.size)
    }

    /// Reissue the transparency hint to the compositor.
    pub fn reload_transparency_hint(&self) {
        let surface = self.window.wl_surface();

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
        match &self.window {
            WindowType::Window { last_configure, .. } => {
                if last_configure.as_ref().map(Self::is_stateless).unwrap_or(true) {
                    self.resize(surface_size.to_logical(self.scale_factor()))
                }
            },
            WindowType::Popup { popup, xdg_positioner, .. } => {
                let size = surface_size.to_logical(self.scale_factor());
                xdg_positioner.set_size(size.width, size.height);
                if popup.xdg_popup().version() >= 3 {
                    popup.reposition(xdg_positioner, 0);
                }
            },
        }

        self.surface_size_physical()
    }

    /// Resize the window to the new surface size.
    fn resize(&mut self, surface_size: LogicalSize<u32>) {
        self.size = surface_size;

        // Update the stateless size.
        if let WindowType::Window { last_configure, .. } = &mut self.window {
            if let Some(true) = last_configure.as_ref().map(Self::is_stateless) {
                self.stateless_size = surface_size;
            }
        }

        // Update the inner frame.
        let ((x, y), outer_size) = if let Some(frame) = self.frame.as_mut() {
            // Resize only visible frame.
            if !frame.is_hidden() {
                frame.resize(
                    NonZeroU32::new(self.size.width).unwrap(),
                    NonZeroU32::new(self.size.height).unwrap(),
                );
            }

            (frame.location(), frame.add_borders(self.size.width, self.size.height).into())
        } else {
            ((0, 0), self.size)
        };

        // Reload the hint.
        self.reload_transparency_hint();

        // Set the window geometry.
        self.window.xdg_surface().set_window_geometry(
            x,
            y,
            outer_size.width as i32,
            outer_size.height as i32,
        );

        // Update the target viewport, this is used if and only if fractional scaling is in use.
        if let Some(viewport) = self.viewport.as_ref() {
            // Set surface size without the borders.
            viewport.set_destination(self.size.width as _, self.size.height as _);
        }

        // Update blur region with new size.
        if self.blur.is_some() {
            // NOTE: either user resized or configure, in both cases
            // the redraw scheduling is done on the caller side.
            let _ = self.set_blur(true);
        }

        self.reload_min_max_hints();
    }

    pub(crate) fn set_maximized(&self, maximized: bool) {
        let Some(xdg_toplevel) = self.window.xdg_toplevel() else { return };

        if maximized { xdg_toplevel.set_maximized() } else { xdg_toplevel.unset_maximized() }
    }

    pub(crate) fn fullscreen(&self) -> Option<Fullscreen> {
        let is_fullscreen = match &self.window {
            WindowType::Window { last_configure, .. } => last_configure
                .as_ref()
                .map(|last_configure| last_configure.is_fullscreen())
                .unwrap_or_default(),
            _ => false,
        };

        if is_fullscreen {
            let current_monitor = self.current_monitor();
            Some(Fullscreen::Borderless(current_monitor))
        } else {
            None
        }
    }

    pub(crate) fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        let Some(xdg_toplevel) = self.xdg_toplevel() else {
            return;
        };
        match fullscreen {
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.as_ref().and_then(|monitor| {
                    monitor.cast_ref::<output::MonitorHandle>().map(|handle| &handle.proxy)
                });

                xdg_toplevel.set_fullscreen(output)
            },
            Some(_) => {
                warn!("this fullscreen mode is ignored on Wayland");
            },
            None => xdg_toplevel.unset_fullscreen(),
        }
    }

    pub(crate) fn is_maximized(&self) -> bool {
        let last_configure = match &self.window {
            WindowType::Window { last_configure, .. } => last_configure,
            WindowType::Popup { .. } => return false,
        };
        last_configure
            .as_ref()
            .map(|last_configure| last_configure.is_maximized())
            .unwrap_or_default()
    }

    /// Get the scale factor of the window.
    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Set the resize increments of the window.
    pub fn set_resize_increments(&mut self, increments: Option<LogicalSize<u32>>) {
        self.resize_increments = increments;
        // NOTE: We don't update the window size here, because it will be done on the next resize
        // or configure event.
    }

    /// Get the resize increments of the window.
    pub fn resize_increments(&self) -> Option<LogicalSize<u32>> {
        self.resize_increments
    }

    /// Set minimum inner window size.
    pub fn set_min_surface_size(&mut self, size: Option<LogicalSize<u32>>) {
        self.min_surface_size = size.unwrap_or_default();
        self.reload_min_max_hints();
    }

    /// Set maximum inner window size.
    pub fn set_max_surface_size(&mut self, size: Option<LogicalSize<u32>>) {
        self.max_surface_size = size;
        self.reload_min_max_hints();
    }

    /// Set the CSD theme.
    pub fn set_theme(&mut self, theme: Option<Theme>) {
        self.theme = theme;
        #[cfg(feature = "sctk-adwaita")]
        if let Some(frame) = self.frame.as_mut() {
            frame.set_config(create_sctk_adwaita_config(theme))
        }
    }

    /// The current theme for CSD decorations.
    #[inline]
    pub fn theme(&self) -> Option<Theme> {
        self.theme
    }

    /// Reload the hints for minimum and maximum sizes.
    pub fn reload_min_max_hints(&mut self) {
        let Some(xdg_toplevel) = self.window.xdg_toplevel() else { return };

        let (mut min, max) = if self.resizable {
            (self.min_surface_size, self.max_surface_size)
        } else {
            (self.stateless_size, Some(self.stateless_size))
        };

        // Ensure that the window has the right minimum size.
        min.width = min.width.max(MIN_WINDOW_SIZE.width);
        min.height = min.height.max(MIN_WINDOW_SIZE.height);

        // Add the borders.
        let add_borders = |size: LogicalSize<u32>| {
            self.frame
                .as_ref()
                .map(|frame| frame.add_borders(size.width, size.height).into())
                .unwrap_or(size)
        };

        let min = add_borders(min);
        xdg_toplevel.set_min_size(min.width as _, min.height as _);
        let max = max.map(add_borders).unwrap_or_default();
        xdg_toplevel.set_max_size(max.width as _, max.height as _);
    }

    pub fn show_window_menu(&self, position: LogicalPosition<u32>) {
        let Some(xdg_toplevel) = self.xdg_toplevel() else { return };

        if let Some((seat, serial)) = &self.touch_down {
            xdg_toplevel.show_window_menu(seat, *serial, position.x as _, position.y as _);
            return;
        }

        self.apply_on_pointer(|_, data| {
            if let Some(serial) = data.latest_button_serial() {
                let seat = data.seat();
                xdg_toplevel.show_window_menu(seat, serial, position.x as _, position.y as _);
            }
        });
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

    /// Set the scale factor for the given window.
    #[inline]
    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;

        // NOTE: When fractional scaling is not used update the buffer scale.
        if self.fractional_scale.is_none() {
            let _ = self.window.set_buffer_scale(self.scale_factor as _);
        }

        if let Some(frame) = self.frame.as_mut() {
            frame.set_scaling_factor(scale_factor);
        }
    }

    /// Make window background blurred.
    ///
    /// Returns `true` if redraw is required.
    #[must_use]
    pub fn set_blur(&mut self, blurred: bool) -> bool {
        if !blurred {
            self.blur = None;
            return true;
        }

        let mgr = match self.blur_manager.as_mut() {
            Some(mgr) => mgr,
            None => {
                info!("Blur manager unavailable, unable to change blur");
                return false;
            },
        };

        let blur = match self.blur.as_ref() {
            Some(blur) => blur,
            None => {
                self.blur = Some(mgr.new_blur_effect(self.window.wl_surface(), &self.queue_handle));
                self.blur.as_ref().unwrap()
            },
        };

        if let Ok(region) = Region::new(&*self.compositor) {
            region.add(0, 0, i32::MAX, i32::MAX);
            blur.set_blur(Some(&region))
        } else {
            false
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
        if let Some(frame) = self.frame.as_mut() {
            frame.set_title(&title);
        }

        if let Some(xdg_toplevel) = self.window.xdg_toplevel() {
            xdg_toplevel.set_title(title.clone());
        }

        self.title = title;
    }

    /// Set the window's icon
    pub fn set_window_icon(&mut self, window_icon: Option<winit_core::icon::Icon>) {
        let Some(xdg_toplevel) = self.xdg_toplevel() else { return };

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

    /// Get the cached title.
    #[inline]
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn children(&self) -> &Vec<WindowId> {
        &self.children
    }

    pub fn parent(&self) -> Option<WindowId> {
        self.parent
    }

    pub fn remove_child(&mut self, child: &WindowId) {
        self.children.retain(|w| w != child);
    }

    pub fn add_child(&mut self, child: WindowId) {
        self.children.push(child);
    }

    pub fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        let data = self.window.wl_surface().data::<SurfaceData<()>>()?;
        data.outputs()
            .next()
            .map(output::MonitorHandle::new)
            .map(|monitor| CoreMonitorHandle(Arc::new(monitor)))
    }

    pub fn set_surface_resize_increments(&mut self, increments: Option<Size>) {
        let increments = increments.map(|size| size.to_logical(self.scale_factor()));
        self.set_resize_increments(increments);
    }

    pub fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        self.resize_increments().map(|size| logical_to_physical_rounded(size, self.scale_factor()))
    }

    pub fn surface_size_physical(&self) -> PhysicalSize<u32> {
        logical_to_physical_rounded(self.surface_size(), self.scale_factor)
    }

    pub fn outer_size_physical(&self) -> PhysicalSize<u32> {
        logical_to_physical_rounded(self.outer_size(), self.scale_factor)
    }
}

impl Drop for WindowState {
    fn drop(&mut self) {
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

#[cfg(feature = "sctk-adwaita")]
fn create_sctk_adwaita_config(theme: Option<Theme>) -> sctk_adwaita::FrameConfig {
    let config = match theme {
        Some(Theme::Light) => sctk_adwaita::FrameConfig::light(),
        Some(Theme::Dark) => sctk_adwaita::FrameConfig::dark(),
        None => sctk_adwaita::FrameConfig::auto(),
    };
    #[cfg(feature = "csd-adwaita-notitlebar")]
    let config = config.hide_titlebar(true);
    config
}
