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

use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::utils::{impl_dyn_casting, AsAny};

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
/// **Web:** A [`MonitorHandle`] created without
#[cfg_attr(
    web_platform,
    doc = "[detailed monitor permissions][crate::platform::web::ActiveEventLoopExtWeb::request_detailed_monitor_permission]."
)]
#[cfg_attr(not(web_platform), doc = "detailed monitor permissions.")]
/// will always represent the current monitor the browser window is in instead of a specific
/// monitor. See
#[cfg_attr(
    web_platform,
    doc = "[`MonitorHandleExtWeb::is_detailed()`][crate::platform::web::MonitorHandleExtWeb::is_detailed]"
)]
#[cfg_attr(not(web_platform), doc = "`MonitorHandleExtWeb::is_detailed()`")]
/// to check.
///
/// [`Window`]: crate::window::Window
#[derive(Debug, Clone)]
pub struct MonitorHandle(pub(crate) Arc<dyn MonitorHandleProvider>);

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
    /// ## Platform-specific
    ///
    /// **Web:** Always returns [`None`] without
    #[cfg_attr(
        web_platform,
        doc = "[detailed monitor permissions][crate::platform::web::ActiveEventLoopExtWeb::request_detailed_monitor_permission]."
    )]
    #[cfg_attr(not(web_platform), doc = "detailed monitor permissions.")]
    fn name(&self) -> Option<Cow<'_, str>>;

    /// Returns the top-left corner position of the monitor in desktop coordinates.
    ///
    /// This position is in the same coordinate system as [`Window::outer_position`].
    ///
    /// [`Window::outer_position`]: crate::window::Window::outer_position
    ///
    /// ## Platform-specific
    ///
    /// **Web:** Always returns [`None`] without
    #[cfg_attr(
        web_platform,
        doc = "[detailed monitor permissions][crate::platform::web::ActiveEventLoopExtWeb::request_detailed_monitor_permission]."
    )]
    #[cfg_attr(not(web_platform), doc = "detailed monitor permissions.")]
    fn position(&self) -> Option<PhysicalPosition<i32>>;

    /// Returns the scale factor of the underlying monitor. To map logical pixels to physical
    /// pixels and vice versa, use [`Window::scale_factor`].
    ///
    /// See the [`dpi`] module for more information.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Can be overridden using the `WINIT_X11_SCALE_FACTOR` environment variable.
    /// - **Wayland:** May differ from [`Window::scale_factor`].
    /// - **Android:** Always returns 1.0.
    /// - **Web:** Always returns `0.0` without
    #[cfg_attr(
        web_platform,
        doc = "  [detailed monitor permissions][crate::platform::web::ActiveEventLoopExtWeb::request_detailed_monitor_permission]."
    )]
    #[cfg_attr(not(web_platform), doc = "  detailed monitor permissions.")]
    ///
    #[rustfmt::skip]
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
