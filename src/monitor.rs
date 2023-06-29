//! Types useful for interacting with a user's monitors.
//!
//! If you want to get basic information about a monitor, you can use the
//! [`MonitorHandle`] type. This is retrieved from one of the following
//! methods, which return an iterator of [`MonitorHandle`]:
//! - [`EventLoopWindowTarget::available_monitors`](crate::event_loop::EventLoopWindowTarget::available_monitors).
//! - [`Window::available_monitors`](crate::window::Window::available_monitors).
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    platform_impl,
};

/// Describes a fullscreen video mode of a monitor.
///
/// Can be acquired with [`MonitorHandle::video_modes`].
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct VideoMode {
    pub(crate) video_mode: platform_impl::VideoMode,
}

impl std::fmt::Debug for VideoMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.video_mode.fmt(f)
    }
}

impl PartialOrd for VideoMode {
    fn partial_cmp(&self, other: &VideoMode) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VideoMode {
    fn cmp(&self, other: &VideoMode) -> std::cmp::Ordering {
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

impl VideoMode {
    /// Returns the resolution of this video mode.
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        self.video_mode.size()
    }

    /// Returns the bit depth of this video mode, as in how many bits you have
    /// available per color. This is generally 24 bits or 32 bits on modern
    /// systems, depending on whether the alpha channel is counted or not.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland / Orbital:** Always returns 32.
    /// - **iOS:** Always returns 32.
    #[inline]
    pub fn bit_depth(&self) -> u16 {
        self.video_mode.bit_depth()
    }

    /// Returns the refresh rate of this video mode in mHz.
    #[inline]
    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.video_mode.refresh_rate_millihertz()
    }

    /// Returns the monitor that this video mode is valid for. Each monitor has
    /// a separate set of valid video modes.
    #[inline]
    pub fn monitor(&self) -> MonitorHandle {
        MonitorHandle {
            inner: self.video_mode.monitor(),
        }
    }
}

impl std::fmt::Display for VideoMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{} @ {} mHz ({} bpp)",
            self.size().width,
            self.size().height,
            self.refresh_rate_millihertz(),
            self.bit_depth()
        )
    }
}

/// Handle to a monitor.
///
/// Allows you to retrieve information about a given monitor and can be used in [`Window`] creation.
///
/// [`Window`]: crate::window::Window
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonitorHandle {
    pub(crate) inner: platform_impl::MonitorHandle,
}

impl MonitorHandle {
    /// Returns a human-readable name of the monitor.
    ///
    /// Returns `None` if the monitor doesn't exist anymore.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** Always returns None
    #[inline]
    pub fn name(&self) -> Option<String> {
        self.inner.name()
    }

    /// Returns the monitor's resolution.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** Always returns (0,0)
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        self.inner.size()
    }

    /// Returns the top-left corner position of the monitor relative to the larger full
    /// screen area.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** Always returns (0,0)
    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        self.inner.position()
    }

    /// The monitor refresh rate used by the system.
    ///
    /// Return `Some` if succeed, or `None` if failed, which usually happens when the monitor
    /// the window is on is removed.
    ///
    /// When using exclusive fullscreen, the refresh rate of the [`VideoMode`] that was used to
    /// enter fullscreen should be used instead.
    #[inline]
    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        self.inner.refresh_rate_millihertz()
    }

    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and vice versa.
    ///
    /// See the [`dpi`](crate::dpi) module for more information.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Can be overridden using the `WINIT_X11_SCALE_FACTOR` environment variable.
    /// - **Android:** Always returns 1.0.
    /// - **Web:** Always returns 1.0
    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    /// Returns all fullscreen video modes supported by this monitor.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** Always returns an empty iterator
    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        self.inner
            .video_modes()
            .map(|video_mode| VideoMode { video_mode })
    }
}
