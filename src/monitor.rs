//! Types useful for interacting with a user's monitors.
use std::fmt;
use std::num::{NonZeroU16, NonZeroU32};

use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::platform_impl;

/// Describes a fullscreen video mode of a monitor.
///
/// Can be retrieved with [`MonitorHandle::video_modes()`].
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
        write!(f, "{}x{}", self.size.width, self.size.height)?;

        if let Some(refresh_rate) = self.refresh_rate_millihertz {
            write!(f, "@{refresh_rate}mHz")?;
        }

        if let Some(bit_depth) = self.bit_depth {
            write!(f, " ({bit_depth} bpp)")?;
        }

        Ok(())
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
    pub fn current_video_mode(&self) -> Option<VideoMode> {
        self.inner.current_video_mode()
    }

    /// Returns all fullscreen video modes supported by this monitor.
    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        self.inner.video_modes()
    }
}
