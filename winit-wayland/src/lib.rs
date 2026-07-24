//! # Winit's Wayland backend.
//!
//! **Note:** Windows don't appear on Wayland until you draw/present to them.
//!
//! By default, Winit loads system libraries using `dlopen`. This can be
//! disabled by disabling the `"wayland-dlopen"` cargo feature.
//!
//! ## Client-side decorations
//!
//! Winit provides client-side decorations by default, but the behaviour can
//! be controlled with the following feature flags:
//!
//! * `wayland-csd-adwaita` (default).
//! * `wayland-csd-adwaita-crossfont`.
//! * `wayland-csd-adwaita-notitle`.
//! * `wayland-csd-adwaita-notitlebar`.

#![allow(clippy::mutable_key_type)]

use std::ffi::c_void;
use std::hash::BuildHasher;
use std::ptr::NonNull;

use dpi::{LogicalSize, PhysicalSize, Position, Size};
use sctk::reexports::client::Proxy;
use sctk::reexports::client::backend::ObjectId;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::shm::slot::{Buffer, CreateBufferError, SlotPool};
use wayland_client::protocol::wl_shm::Format;
use winit_core::data_transfer::DataTransferId;
use winit_core::event_loop::ActiveEventLoop as CoreActiveEventLoop;
use winit_core::window::{
    ActivationToken, PlatformWindowAttributes, Window as CoreWindow, WindowId,
};

macro_rules! os_error {
    ($error:expr) => {{ winit_core::error::OsError::new(line!(), file!(), $error) }};
}

mod dnd;
mod event_loop;
mod output;
mod popup;
mod seat;
mod state;
mod types;
mod window;

pub use self::dnd::{DataOffer, DragSource, MimeData, MimeType};
pub use self::event_loop::{ActiveEventLoop, EventLoop};
pub use self::popup::Popup;
pub use self::window::Window;

/// Additional methods on [`ActiveEventLoop`] that are specific to Wayland.
pub trait ActiveEventLoopExtWayland {
    /// True if the [`ActiveEventLoop`] uses Wayland.
    fn is_wayland(&self) -> bool;
}

impl ActiveEventLoopExtWayland for dyn CoreActiveEventLoop + '_ {
    #[inline]
    fn is_wayland(&self) -> bool {
        self.cast_ref::<ActiveEventLoop>().is_some()
    }
}

/// Additional methods on [`EventLoop`] that are specific to Wayland.
pub trait EventLoopExtWayland {
    /// True if the [`EventLoop`] uses Wayland.
    fn is_wayland(&self) -> bool;
}

/// Additional methods when building event loop that are specific to Wayland.
pub trait EventLoopBuilderExtWayland {
    /// Force using Wayland.
    fn with_wayland(&mut self) -> &mut Self;

    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;
}

/// Additional methods on [`Window`] that are specific to Wayland.
///
/// [`Window`]: crate::window::Window
pub trait WindowExtWayland {
    /// Returns `xdg_toplevel` of the window or [`None`] if the window is X11 window.
    fn xdg_toplevel(&self) -> Option<NonNull<c_void>>;
}

impl WindowExtWayland for dyn CoreWindow + '_ {
    #[inline]
    fn xdg_toplevel(&self) -> Option<NonNull<c_void>> {
        self.cast_ref::<Window>()?.xdg_toplevel()
    }
}

/// Additional methods on [`Popup`] that are specific to Wayland.
pub trait PopupExtWayland {
    fn anchor_rect(&self) -> Option<(impl Into<Position>, impl Into<Size>)>;

    /// Sets the anchor edge of the parent surface the popup is positioned relative to.
    ///
    /// See [`PopupAnchor`] for the available edges and corners.
    fn set_anchor(&self, anchor: PopupAnchor);

    /// Sets the anchor rectangle within the parent surface the popup is positioned relative to.
    ///
    /// `position` is the top-left corner of the rectangle relative to the parent window's content
    /// area, and `size` its dimensions.
    fn set_anchor_rect(&self, position: impl Into<Position>, size: impl Into<Size>);

    /// Sets how the compositor should reposition the popup when it would be constrained by screen
    /// edges.
    ///
    /// See [`PopupConstraintAdjustment`] for the available adjustment flags.
    fn set_constraint_adjustment(&self, constraint_adjustment: PopupConstraintAdjustment);

    /// Sets the direction the popup surface extends from the anchor point.
    ///
    /// See [`PopupGravity`] for the available directions.
    fn set_gravity(&self, gravity: PopupGravity);

    /// Set the popup position relative to the anchor rect
    fn set_positioner_offset(&self, position: impl Into<Position>);
}

impl PopupExtWayland for dyn CoreWindow + '_ {
    fn set_anchor(&self, anchor: PopupAnchor) {
        if let Some(popup) = self.cast_ref::<Popup>() {
            popup.set_anchor(anchor);
        }
    }

    fn anchor_rect(&self) -> Option<(impl Into<Position>, impl Into<Size>)> {
        if let Some(popup) = self.cast_ref::<Popup>() { popup.anchor_rect() } else { None }
    }

    fn set_anchor_rect(&self, position: impl Into<Position>, size: impl Into<Size>) {
        if let Some(popup) = self.cast_ref::<Popup>() {
            popup.set_anchor_rect(position, size);
        }
    }

    fn set_constraint_adjustment(&self, constraint_adjustment: PopupConstraintAdjustment) {
        if let Some(popup) = self.cast_ref::<Popup>() {
            popup.set_constraint_adjustment(constraint_adjustment);
        }
    }

    fn set_gravity(&self, gravity: PopupGravity) {
        if let Some(popup) = self.cast_ref::<Popup>() {
            popup.set_gravity(gravity);
        }
    }

    fn set_positioner_offset(&self, position: impl Into<Position>) {
        if let Some(popup) = self.cast_ref::<Popup>() {
            popup.set_positioner_offset(position);
        }
    }
}

/// Anchor rect within the parent surface
/// See: https://wayland.app/protocols/xdg-shell#xdg_positioner:request:set_anchor_rect
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[non_exhaustive]
pub enum PopupAnchor {
    #[default]
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

/// Defines in what direction a surface should be positioned
/// See: https://wayland.app/protocols/xdg-shell#xdg_positioner:request:set_gravity
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[non_exhaustive]
pub enum PopupGravity {
    #[default]
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

bitflags::bitflags! {
    /// Specify how the window should be positioned if the originally intended position caused the
    /// surface to be constrained See: https://wayland.app/protocols/xdg-shell#xdg_positioner:request:set_constraint_adjustment
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub struct PopupConstraintAdjustment: u32 {
        const SLIDE_X = 1 << 0;
        const SLIDE_Y = 1 << 1;
        const FLIP_X = 1 << 2;
        const FLIP_Y = 1 << 3;
        const RESIZE_X = 1 << 4;
        const RESIZE_Y = 1 << 5;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplicationName {
    pub(crate) general: String,
    pub(crate) instance: String,
}

/// Window attributes methods specific to Wayland.
#[derive(Debug, Default, Clone)]
pub struct WindowAttributesWayland {
    pub(crate) name: Option<ApplicationName>,
    pub(crate) activation_token: Option<ActivationToken>,
    pub(crate) prefer_csd: bool,
    pub(crate) anchor: Option<PopupAnchor>,
    pub(crate) anchor_rect: Option<(Position, Size)>,
    /// Only for Popup: The offset of the popup to the
    /// anchor rect
    pub(crate) positioner_offset: Option<Position>,
    pub(crate) gravity: Option<PopupGravity>,
    pub(crate) constraint_adjustment: Option<PopupConstraintAdjustment>,
}

impl WindowAttributesWayland {
    /// Build window with the given name.
    ///
    /// The `general` name sets an application ID, which should match the `.desktop`
    /// file distributed with your program. The `instance` is a `no-op`.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    #[inline]
    pub fn with_name(mut self, general: impl Into<String>, instance: impl Into<String>) -> Self {
        self.name = Some(ApplicationName { general: general.into(), instance: instance.into() });
        self
    }

    /// Sets an activation token to use when creating the window.
    ///
    /// The activation token allows the compositor to grant focus to the new window,
    /// overriding focus-stealing prevention. Obtain a token via
    /// [`ActiveEventLoop::request_activation_token`].
    #[inline]
    pub fn with_activation_token(mut self, token: ActivationToken) -> Self {
        self.activation_token = Some(token);
        self
    }

    /// Builds the window with a given preference for client-side decorations.
    ///
    /// When set to `true`, the window will tell the compositor that it prefers
    /// client-side decorations, even if server-side decorations are available.
    /// When set to `false` (the default), the window will indicate a preference
    /// for server-side decorations.
    #[inline]
    pub fn with_prefer_csd(mut self, prefer_csd: bool) -> Self {
        self.prefer_csd = prefer_csd;
        self
    }

    /// Sets the edge or corner of the anchor rectangle the popup is attached to.
    ///
    /// Combined with [`with_gravity`](Self::with_gravity), this controls which corner/edge of the
    /// anchor rectangle the popup is pinned to. Has no effect unless the window is created as a
    /// [`Popup`](winit_core::window::WindowType::Popup).
    #[inline]
    pub fn with_anchor(mut self, anchor: PopupAnchor) -> Self {
        self.anchor = Some(anchor);
        self
    }

    /// Set the anchor rectangle the popup is positioned relative to.
    ///
    /// `position` is the top-left corner of the rectangle relative to the parent window's content
    /// area, and `size` its dimensions. Defaults to a `1x1` rectangle at the content origin.
    /// This value overwrites the position value set with `with_position` in the window attributes
    #[inline]
    pub fn with_anchor_rect(
        mut self,
        position: impl Into<Position>,
        size: impl Into<Size>,
    ) -> Self {
        self.anchor_rect = Some((position.into(), size.into()));
        self
    }

    /// Set the popup position relative to the anchor rect
    pub fn with_positioner_offset(mut self, position: impl Into<Position>) -> Self {
        self.positioner_offset = Some(position.into());
        self
    }

    /// Sets how the compositor should reposition the popup when it would be constrained.
    ///
    /// The flags in [`PopupConstraintAdjustment`] can be combined to allow sliding, flipping,
    /// and/or resizing the popup independently on each axis. Has no effect unless the window is
    /// created as a [`Popup`](winit_core::window::WindowType::Popup).
    #[inline]
    pub fn with_constraint_adjustment(
        mut self,
        constraint_adjustment: PopupConstraintAdjustment,
    ) -> Self {
        self.constraint_adjustment = Some(constraint_adjustment);
        self
    }

    /// Sets the direction the popup surface extends away from the anchor point.
    ///
    /// Combined with [`with_anchor`](Self::with_anchor), this determines the final position of the
    /// popup relative to its anchor rectangle. Has no effect unless the window is created as a
    /// [`Popup`](winit_core::window::WindowType::Popup).
    #[inline]
    pub fn with_gravity(mut self, gravity: PopupGravity) -> Self {
        self.gravity = Some(gravity);
        self
    }
}

impl PlatformWindowAttributes for WindowAttributesWayland {
    fn box_clone(&self) -> Box<dyn PlatformWindowAttributes> {
        Box::from(self.clone())
    }
}

/// Get the WindowId out of the surface.
#[inline]
fn make_wid(surface: &WlSurface) -> WindowId {
    WindowId::from_raw(surface.id().as_ptr() as usize)
}

/// Create a `DataTransferId` for the given data device and serial.
///
/// It's currently unclear if this will result in the same ID when transferring to the same
/// application.
#[inline]
fn make_data_transfer_id(data_device_id: ObjectId, serial: u32) -> DataTransferId {
    const BUILD_HASHER: foldhash::fast::FixedState = foldhash::fast::FixedState::with_seed(0);

    DataTransferId::from_raw(BUILD_HASHER.hash_one((data_device_id, serial)) as i64)
}

/// The default routine does floor, but we need round on Wayland.
fn logical_to_physical_rounded(size: LogicalSize<u32>, scale_factor: f64) -> PhysicalSize<u32> {
    let width = size.width as f64 * scale_factor;
    let height = size.height as f64 * scale_factor;
    (width.round(), height.round()).into()
}

/// Converts an image buffer to a Wayland buffer (`wl_buffer`)
fn image_to_buffer(
    width: i32,
    height: i32,
    data: &[u8],
    format: Format,
    pool: &mut SlotPool,
) -> Result<Buffer, CreateBufferError> {
    let (buffer, canvas) = pool.create_buffer(width, height, 4 * width, format)?;

    for (canvas_chunk, rgba) in canvas.chunks_exact_mut(4).zip(data.chunks_exact(4)) {
        // Alpha in buffer is premultiplied.
        let alpha = rgba[3] as f32 / 255.;
        let r = (rgba[0] as f32 * alpha) as u32;
        let g = (rgba[1] as f32 * alpha) as u32;
        let b = (rgba[2] as f32 * alpha) as u32;
        let color = ((rgba[3] as u32) << 24) + (r << 16) + (g << 8) + b;
        let array: &mut [u8; 4] = canvas_chunk.try_into().unwrap();
        *array = color.to_le_bytes();
    }

    Ok(buffer)
}

impl From<PopupGravity> for wayland_protocols::xdg::shell::client::xdg_positioner::Gravity {
    fn from(value: PopupGravity) -> Self {
        use wayland_protocols::xdg::shell::client::xdg_positioner::Gravity;
        match value {
            PopupGravity::None => Gravity::None,
            PopupGravity::Top => Gravity::Top,
            PopupGravity::Bottom => Gravity::Bottom,
            PopupGravity::Left => Gravity::Left,
            PopupGravity::Right => Gravity::Right,
            PopupGravity::TopLeft => Gravity::TopLeft,
            PopupGravity::BottomLeft => Gravity::BottomLeft,
            PopupGravity::TopRight => Gravity::TopRight,
            PopupGravity::BottomRight => Gravity::BottomRight,
        }
    }
}

impl From<PopupAnchor> for wayland_protocols::xdg::shell::client::xdg_positioner::Anchor {
    fn from(value: PopupAnchor) -> Self {
        use wayland_protocols::xdg::shell::client::xdg_positioner::Anchor;
        match value {
            PopupAnchor::None => Anchor::None,
            PopupAnchor::Top => Anchor::Top,
            PopupAnchor::Bottom => Anchor::Bottom,
            PopupAnchor::Left => Anchor::Left,
            PopupAnchor::Right => Anchor::Right,
            PopupAnchor::TopLeft => Anchor::TopLeft,
            PopupAnchor::BottomLeft => Anchor::BottomLeft,
            PopupAnchor::TopRight => Anchor::TopRight,
            PopupAnchor::BottomRight => Anchor::BottomRight,
        }
    }
}

impl From<PopupConstraintAdjustment>
    for wayland_protocols::xdg::shell::client::xdg_positioner::ConstraintAdjustment
{
    fn from(value: PopupConstraintAdjustment) -> Self {
        use wayland_protocols::xdg::shell::client::xdg_positioner::ConstraintAdjustment;

        const _: () = {
            assert!(
                PopupConstraintAdjustment::SLIDE_X.bits() == ConstraintAdjustment::SlideX.bits()
            );
            assert!(
                PopupConstraintAdjustment::SLIDE_Y.bits() == ConstraintAdjustment::SlideY.bits()
            );
            assert!(PopupConstraintAdjustment::FLIP_X.bits() == ConstraintAdjustment::FlipX.bits());
            assert!(PopupConstraintAdjustment::FLIP_Y.bits() == ConstraintAdjustment::FlipY.bits());
            assert!(
                PopupConstraintAdjustment::RESIZE_X.bits() == ConstraintAdjustment::ResizeX.bits()
            );
            assert!(
                PopupConstraintAdjustment::RESIZE_Y.bits() == ConstraintAdjustment::ResizeY.bits()
            );
        };

        ConstraintAdjustment::from_bits_retain(value.bits())
    }
}
