//! Types useful for interacting with a user's monitors.
use std::num::{NonZeroU16, NonZeroU32};

use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::platform_impl;

/// A handle to a fullscreen video mode of a specific monitor.
///
/// This can be acquired with [`MonitorHandle::video_modes`].
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct VideoModeHandle {
    pub(crate) video_mode: platform_impl::VideoModeHandle,
}

impl std::fmt::Debug for VideoModeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.video_mode.fmt(f)
    }
}

impl PartialOrd for VideoModeHandle {
    fn partial_cmp(&self, other: &VideoModeHandle) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VideoModeHandle {
    fn cmp(&self, other: &VideoModeHandle) -> std::cmp::Ordering {
        self.monitor().cmp(&other.monitor()).then(
            self.size()
                .cmp(&other.size())
                .then(
                    self.refresh_rate_millihertz()
                        .cmp(&other.refresh_rate_millihertz())
                        .then(self.bit_depth().cmp(&other.bit_depth())),
                )
                .reverse(),
        )
    }
}

impl VideoModeHandle {
    /// Returns the resolution of this video mode. This **must not** be used to create your
    /// rendering surface. Use [`Window::surface_size()`] instead.
    ///
    /// [`Window::surface_size()`]: crate::window::Window::surface_size
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        self.video_mode.size()
    }

    /// Returns the bit depth of this video mode, as in how many bits you have
    /// available per color. This is generally 24 bits or 32 bits on modern
    /// systems, depending on whether the alpha channel is counted or not.
    #[inline]
    pub fn bit_depth(&self) -> Option<NonZeroU16> {
        self.video_mode.bit_depth()
    }

    /// Returns the refresh rate of this video mode in mHz.
    #[inline]
    pub fn refresh_rate_millihertz(&self) -> Option<NonZeroU32> {
        self.video_mode.refresh_rate_millihertz()
    }

    /// Returns the monitor that this video mode is valid for. Each monitor has
    /// a separate set of valid video modes.
    #[inline]
    pub fn monitor(&self) -> MonitorHandle {
        MonitorHandle { inner: self.video_mode.monitor() }
    }
}

impl std::fmt::Display for VideoModeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{} {}{}",
            self.size().width,
            self.size().height,
            self.refresh_rate_millihertz().map(|rate| format!("@ {rate} mHz ")).unwrap_or_default(),
            self.bit_depth().map(|bit_depth| format!("({bit_depth} bpp)")).unwrap_or_default(),
        )
    }
}

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
    any(web_platform, docsrs),
    doc = "[detailed monitor permissions][crate::platform::web::ActiveEventLoopExtWeb::request_detailed_monitor_permission]."
)]
#[cfg_attr(not(any(web_platform, docsrs)), doc = "detailed monitor permissions.")]
/// will always represent the current monitor the browser window is in instead of a specific
/// monitor. See
#[cfg_attr(
    any(web_platform, docsrs),
    doc = "[`MonitorHandleExtWeb::is_detailed()`][crate::platform::web::MonitorHandleExtWeb::is_detailed]"
)]
#[cfg_attr(not(any(web_platform, docsrs)), doc = "`MonitorHandleExtWeb::is_detailed()`")]
/// to check.
///
/// [`Window`]: crate::window::Window
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MonitorHandle {
    pub(crate) inner: platform_impl::MonitorHandle,
}

impl std::fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl MonitorHandle {
    /// Returns a human-readable name of the monitor.
    ///
    /// Returns `None` if the monitor doesn't exist anymore.
    ///
    /// ## Platform-specific
    ///
    /// **Web:** Always returns [`None`] without
    #[cfg_attr(
        any(web_platform, docsrs),
        doc = "[detailed monitor permissions][crate::platform::web::ActiveEventLoopExtWeb::request_detailed_monitor_permission]."
    )]
    #[cfg_attr(not(any(web_platform, docsrs)), doc = "detailed monitor permissions.")]
    #[inline]
    pub fn name(&self) -> Option<String> {
        self.inner.name()
    }

    /// Returns the top-left corner position of the monitor relative to the larger full
    /// screen area.
    ///
    /// ## Platform-specific
    ///
    /// **Web:** Always returns [`None`] without
    #[cfg_attr(
        any(web_platform, docsrs),
        doc = "[detailed monitor permissions][crate::platform::web::ActiveEventLoopExtWeb::request_detailed_monitor_permission]."
    )]
    #[cfg_attr(not(any(web_platform, docsrs)), doc = "detailed monitor permissions.")]
    #[inline]
    pub fn position(&self) -> Option<PhysicalPosition<i32>> {
        self.inner.position()
    }

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
        any(web_platform, docsrs),
        doc = "  [detailed monitor permissions][crate::platform::web::ActiveEventLoopExtWeb::request_detailed_monitor_permission]."
    )]
    #[cfg_attr(not(any(web_platform, docsrs)), doc = "  detailed monitor permissions.")]
    ///
    #[rustfmt::skip]
    /// [`Window::scale_factor`]: crate::window::Window::scale_factor
    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    /// Returns the currently active video mode of this monitor.
    #[inline]
    pub fn current_video_mode(&self) -> Option<VideoModeHandle> {
        self.inner.current_video_mode().map(|video_mode| VideoModeHandle { video_mode })
    }

    /// Returns all fullscreen video modes supported by this monitor.
    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = VideoModeHandle> {
        self.inner.video_modes().map(|video_mode| VideoModeHandle { video_mode })
    }
}
