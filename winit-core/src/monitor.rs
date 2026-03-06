//! Types useful for interacting with a user's monitors.
//!
//! If you want to get basic information about a monitor, you can use the
//! [`MonitorHandle`] type. This is retrieved from one of the following
//! methods, which return an iterator of [`MonitorHandle`]:
//! - [`ActiveEventLoop::available_monitors`][crate::event_loop::ActiveEventLoop::available_monitors].
//! - [`Window::available_monitors`][crate::window::Window::available_monitors].
use std::borrow::Cow;
use std::fmt;
use std::num::{NonZeroU16, NonZeroU32};
use std::ops::Deref;
use std::sync::Arc;

use dpi::{PhysicalPosition, PhysicalSize, Position};

use crate::as_any::AsAny;

/// Handle to a monitor.
///
/// Allows you to retrieve basic information and metadata about a monitor.
///
/// Can be used in [`Window`] creation to place the window on a specific
/// monitor.
///
/// This can be retrieved from one of the following methods, which return an
/// iterator of [`MonitorHandle`]s:
/// - [`ActiveEventLoop::available_monitors`](crate::event_loop::ActiveEventLoop::available_monitors).
/// - [`Window::available_monitors`](crate::window::Window::available_monitors).
///
/// ## Platform-specific
///
/// **Web:** A [`MonitorHandle`] created without `detailed monitor permissions`
/// will always represent the current monitor the browser window is in instead of a specific
/// monitor.
///
/// [`Window`]: crate::window::Window
#[derive(Debug, Clone)]
pub struct MonitorHandle(pub Arc<dyn MonitorHandleProvider>);

impl Deref for MonitorHandle {
    type Target = dyn MonitorHandleProvider;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ref().eq(other.0.as_ref())
    }
}

impl Eq for MonitorHandle {}

/// Provider of the [`MonitorHandle`].
pub trait MonitorHandleProvider: AsAny + fmt::Debug + Send + Sync {
    /// Identifier for this monitor.
    ///
    /// The representation of this modifier is not guaranteed and should be used only to compare
    /// monitors.
    fn id(&self) -> u128;

    /// Native platform identifier of this monitor.
    ///
    /// # Platform-specific
    ///
    /// - **Windows**: This is `HMONITOR`.
    /// - **macOS**: This is `CGDirectDisplayID`.
    /// - **iOS**: This is `UIScreen*`.
    /// - **Wayland**: This is the ID of the `wl_output` device.
    /// - **X11**: This is the ID of the CRTC.
    /// - **Web**: This is an internal ID not meant for consumption.
    fn native_id(&self) -> u64;

    /// Returns a human-readable name of the monitor.
    ///
    /// Returns `None` if the monitor doesn't exist anymore or the name couldn't be obtained.
    ///
    ///
    /// ## Platform-specific
    ///
    /// **Web:** Always returns [`None`] without `detailed monitor permissions`.
    fn name(&self) -> Option<Cow<'_, str>>;

    /// Returns the top-left corner position of the monitor in desktop coordinates.
    ///
    /// This position is in the same coordinate system as [`Window::outer_position`].
    ///
    /// [`Window::outer_position`]: crate::window::Window::outer_position
    ///
    /// ## Platform-specific
    ///
    /// **Web:** Always returns [`None`] without `detailed monitor permissions`.
    fn position(&self) -> Option<PhysicalPosition<i32>>;

    /// Returns the scale factor of the underlying monitor. To map logical pixels to physical
    /// pixels and vice versa, use [`Window::scale_factor`].
    ///
    /// See the [`dpi`] module for more information.
    ///
    /// - **Wayland:** May differ from [`Window::scale_factor`].
    /// - **Web:** Always returns `0.0` without `detailed_monitor_permissions`.
    ///
    /// [`Window::scale_factor`]: crate::window::Window::scale_factor
    fn scale_factor(&self) -> f64;

    fn current_video_mode(&self) -> Option<VideoMode>;

    /// Returns all fullscreen video modes supported by this monitor.
    fn video_modes(&self) -> Box<dyn Iterator<Item = VideoMode>>;
}

impl PartialEq for dyn MonitorHandleProvider + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for dyn MonitorHandleProvider + '_ {}

impl_dyn_casting!(MonitorHandleProvider);

/// Describes a fullscreen video mode of a monitor.
///
/// Can be acquired with [`MonitorHandleProvider::video_modes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoMode {
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) bit_depth: Option<NonZeroU16>,
    pub(crate) refresh_rate_millihertz: Option<NonZeroU32>,
}

impl VideoMode {
    pub fn new(
        size: PhysicalSize<u32>,
        bit_depth: Option<NonZeroU16>,
        refresh_rate_millihertz: Option<NonZeroU32>,
    ) -> Self {
        Self { size, bit_depth, refresh_rate_millihertz }
    }

    /// Returns the resolution of this video mode. This **must not** be used to create your
    /// rendering surface. Use [`Window::surface_size()`] instead.
    ///
    /// [`Window::surface_size()`]: crate::window::Window::surface_size
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    /// Returns the bit depth of this video mode, as in how many bits you have
    /// available per color. This is generally 24 bits or 32 bits on modern
    /// systems, depending on whether the alpha channel is counted or not.
    ///
    /// # Platform-specific
    ///
    /// - **macOS**: Video modes do not control the bit depth of the monitor, so this often defaults
    ///   to 32.
    /// - **iOS**: Always returns `None`.
    /// - **Wayland**: Always returns `None`.
    pub fn bit_depth(&self) -> Option<NonZeroU16> {
        self.bit_depth
    }

    /// Returns the refresh rate of this video mode in mHz.
    pub fn refresh_rate_millihertz(&self) -> Option<NonZeroU32> {
        self.refresh_rate_millihertz
    }
}

impl fmt::Display for VideoMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{} {}{}",
            self.size.width,
            self.size.height,
            self.refresh_rate_millihertz.map(|rate| format!("@ {rate} mHz ")).unwrap_or_default(),
            self.bit_depth.map(|bit_depth| format!("({bit_depth} bpp)")).unwrap_or_default(),
        )
    }
}

/// Fullscreen modes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fullscreen {
    Exclusive(MonitorHandle, VideoMode),

    /// Providing `None` to `Borderless` will fullscreen on the current monitor.
    Borderless(Option<MonitorHandle>),
}

/// A monitor's logical bounds and scale factor, used for determining which
/// monitor a physical or logical position targets.
#[derive(Debug, Clone, Copy)]
pub struct MonitorBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

impl MonitorBounds {
    /// Create `MonitorBounds` from physical position, physical size, and scale factor.
    ///
    /// All platforms report monitor position and size in physical pixels. This
    /// constructor converts to logical coordinates by dividing by the scale factor,
    /// producing the uniform logical-space representation that
    /// [`resolve_scale_factor`] expects.
    pub fn from_physical(
        position: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
        scale: f64,
    ) -> Self {
        Self {
            x: position.x as f64 / scale,
            y: position.y as f64 / scale,
            width: size.width as f64 / scale,
            height: size.height as f64 / scale,
            scale,
        }
    }
}

/// Determine the scale factor of the target monitor for a given position.
///
/// Monitor bounds are in logical coordinates. For `Physical` positions, each
/// monitor's scale factor is used to convert to logical before checking bounds.
/// For `Logical` positions, bounds are checked directly.
///
/// Returns `None` if no monitor contains the position.
pub fn resolve_scale_factor(position: &Position, monitors: &[MonitorBounds]) -> Option<f64> {
    for monitor in monitors {
        if monitor.width <= 0.0 || monitor.scale <= 0.0 {
            continue;
        }

        let (logical_x, logical_y) = match position {
            Position::Physical(p) => (p.x as f64 / monitor.scale, p.y as f64 / monitor.scale),
            Position::Logical(l) => (l.x, l.y),
        };

        if logical_x >= monitor.x
            && logical_x < monitor.x + monitor.width
            && logical_y >= monitor.y
            && logical_y < monitor.y + monitor.height
        {
            return Some(monitor.scale);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use dpi::{LogicalPosition, PhysicalPosition};

    use super::*;

    fn monitor(x: f64, y: f64, width: f64, height: f64, scale: f64) -> MonitorBounds {
        MonitorBounds { x, y, width, height, scale }
    }

    fn physical(x: i32, y: i32) -> Position {
        Position::Physical(PhysicalPosition::new(x, y))
    }

    fn logical(x: f64, y: f64) -> Position {
        Position::Logical(LogicalPosition::new(x, y))
    }

    // Typical macOS layout: Retina MacBook (2x) as primary at origin,
    // external 1x display to the upper-left.
    //
    // CG coordinate space (logical points, origin top-left of primary):
    //   Monitor 0 (2x): (0, 0) 1728x1117
    //   Monitor 1 (1x): (-816, -1440) 3440x1440
    fn macos_layout() -> Vec<MonitorBounds> {
        vec![monitor(0.0, 0.0, 1728.0, 1117.0, 2.0), monitor(-816.0, -1440.0, 3440.0, 1440.0, 1.0)]
    }

    #[test]
    fn physical_position_on_2x_monitor() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&physical(400, 600), &monitors);
        assert_eq!(result, Some(2.0));
    }

    #[test]
    fn physical_position_on_1x_monitor() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&physical(-500, -1000), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn high_to_low_cross_monitor_restore() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&physical(-716, -1340), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn low_to_high_cross_monitor_restore() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&physical(400, 400), &monitors);
        assert_eq!(result, Some(2.0));
    }

    #[test]
    fn physical_position_outside_all_monitors_returns_none() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&physical(99999, 99999), &monitors);
        assert_eq!(result, None);
    }

    #[test]
    fn single_monitor_always_matches() {
        let monitors = vec![monitor(0.0, 0.0, 1920.0, 1080.0, 1.0)];
        let result = resolve_scale_factor(&physical(500, 300), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn single_retina_monitor() {
        let monitors = vec![monitor(0.0, 0.0, 1728.0, 1117.0, 2.0)];
        let result = resolve_scale_factor(&physical(1000, 800), &monitors);
        assert_eq!(result, Some(2.0));
    }

    #[test]
    fn three_monitors_mixed_scales() {
        let monitors = vec![
            monitor(-1920.0, 0.0, 1920.0, 1080.0, 1.0),
            monitor(0.0, 0.0, 1728.0, 1117.0, 2.0),
            monitor(1728.0, 0.0, 2560.0, 1440.0, 1.5),
        ];

        let result = resolve_scale_factor(&physical(-1000, 500), &monitors);
        assert_eq!(result, Some(1.0));

        let result = resolve_scale_factor(&physical(1000, 800), &monitors);
        assert_eq!(result, Some(2.0));

        let result = resolve_scale_factor(&physical(5000, 600), &monitors);
        assert_eq!(result, Some(1.5));
    }

    #[test]
    fn zero_width_monitor_is_skipped() {
        let monitors =
            vec![monitor(0.0, 0.0, 0.0, 0.0, 2.0), monitor(0.0, 0.0, 1920.0, 1080.0, 1.0)];
        let result = resolve_scale_factor(&physical(500, 300), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn zero_scale_monitor_is_skipped() {
        let monitors =
            vec![monitor(0.0, 0.0, 1920.0, 1080.0, 0.0), monitor(0.0, 0.0, 1920.0, 1080.0, 1.0)];
        let result = resolve_scale_factor(&physical(500, 300), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn empty_monitor_list_returns_none() {
        let result = resolve_scale_factor(&physical(500, 300), &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn logical_position_on_2x_monitor() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&logical(200.0, 300.0), &monitors);
        assert_eq!(result, Some(2.0));
    }

    #[test]
    fn logical_position_on_1x_monitor() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&logical(-500.0, -1000.0), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn logical_position_outside_all_monitors_returns_none() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&logical(99999.0, 99999.0), &monitors);
        assert_eq!(result, None);
    }

    #[test]
    fn logical_position_three_monitors_mixed_scales() {
        let monitors = vec![
            monitor(-1920.0, 0.0, 1920.0, 1080.0, 1.0),
            monitor(0.0, 0.0, 1728.0, 1117.0, 2.0),
            monitor(1728.0, 0.0, 2560.0, 1440.0, 1.5),
        ];

        let result = resolve_scale_factor(&logical(-1000.0, 500.0), &monitors);
        assert_eq!(result, Some(1.0));

        let result = resolve_scale_factor(&logical(500.0, 500.0), &monitors);
        assert_eq!(result, Some(2.0));

        let result = resolve_scale_factor(&logical(2000.0, 500.0), &monitors);
        assert_eq!(result, Some(1.5));
    }

    #[test]
    fn logical_empty_monitor_list_returns_none() {
        let result = resolve_scale_factor(&logical(500.0, 300.0), &[]);
        assert_eq!(result, None);
    }
}
