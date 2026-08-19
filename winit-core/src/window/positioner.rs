//! Anchor-based window placement: the types describing a placement request.
//!
//! The `xdg_positioner`-style algorithm that resolves these into a concrete position and size
//! lives in `winit-common`, since it's only needed by backends without a native equivalent of
//! Wayland's `xdg_positioner` (such as Win32 and AppKit) and isn't part of winit's public API.

/// Anchor rect within the parent surface
/// See: https://wayland.app/protocols/xdg-shell#xdg_positioner:request:set_anchor_rect
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[non_exhaustive]
pub enum WindowAnchor {
    #[default]
    Center,
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
pub enum WindowGravity {
    #[default]
    Center,
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
    /// For all other platforms than wayland the behaviour is simulated on the winit side
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub struct WindowConstraintAdjustment: u32 {
        const SLIDE_X = 1 << 0;
        const SLIDE_Y = 1 << 1;
        const FLIP_X = 1 << 2;
        const FLIP_Y = 1 << 3;
        const RESIZE_X = 1 << 4;
        const RESIZE_Y = 1 << 5;
    }
}
