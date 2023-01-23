//! Types useful for interacting with a user's monitors.
//!
//! If you want to get basic information about a monitor, you can use the
//! [`MonitorHandle`] type. This is retrieved from one of the following
//! methods, which return an iterator of [`MonitorHandle`]:
//! - [`EventLoopWindowTarget::available_monitors`](crate::event_loop::EventLoopWindowTarget::available_monitors).
//! - [`Window::available_monitors`](crate::window::Window::available_monitors).
use std::error;
use std::fmt;

use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    platform_impl,
};

/// Describes a fullscreen video mode of a monitor.
///
/// A list of these can be acquired with [`MonitorHandle::video_modes`].
///
/// `VideoMode` is essentially just a static blob of data, and it's properties
/// will _not_ be updated automatically if a video mode changes - refetch the
/// modes instead.
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
        // TODO: we can impl `Ord` for `PhysicalSize` once we switch from `f32`
        // to `u32` there
        let size: (u32, u32) = self.size().into();
        let other_size: (u32, u32) = other.size().into();
        self.monitor().cmp(&other.monitor()).then(
            size.cmp(&other_size)
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
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland / Orbital / iOS:** Always returns 32.
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

/// A handle to a monitor.
///
/// Allows you to retrieve information about a given monitor, and can be used
/// in [`Window`] creation to set the monitor that the window will go
/// fullscreen on.
///
/// Since a monitor can be removed by the user at any time, all methods on
/// this return a [`Result`] with a [`MonitorGone`] as the error case.
///
/// [`Window`]: crate::window::Window
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonitorHandle {
    pub(crate) inner: platform_impl::MonitorHandle,
}

/// Signifies that the monitor isn't connected to the system anymore.
///
/// This might sometimes also happen if the monitors parameters changed enough
/// that the system deemed it should destroy and recreate the handle to it.
///
/// Finally, it may happen spuriously around the time when a monitor is
/// reconnected.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorGone {
    _inner: (),
}

impl MonitorGone {
    #[allow(dead_code)]
    pub(crate) fn new() -> Self {
        Self { _inner: () }
    }
}

impl fmt::Display for MonitorGone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the monitor could not be found (it might have been disconnected)"
        )
    }
}

impl error::Error for MonitorGone {}

impl MonitorHandle {
    /// A human-readable name of the monitor.
    #[inline]
    pub fn name(&self) -> Result<String, MonitorGone> {
        self.inner.name()
    }

    /// The monitor's currently configured resolution.
    #[inline]
    pub fn size(&self) -> Result<PhysicalSize<u32>, MonitorGone> {
        self.inner.size()
    }

    /// The current position of the monitor in the desktop at large.
    ///
    /// The position has origin in the top-left of the monitor.
    #[inline]
    pub fn position(&self) -> Result<PhysicalPosition<i32>, MonitorGone> {
        self.inner.position()
    }

    /// The refresh rate currently in use on the monitor.
    ///
    /// When using exclusive fullscreen, the refresh rate of the [`VideoMode`] that was used to
    /// enter fullscreen should be used instead.
    #[inline]
    pub fn refresh_rate_millihertz(&self) -> Result<u32, MonitorGone> {
        self.inner.refresh_rate_millihertz()
    }

    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and vice versa.
    ///
    /// See the [`dpi`](crate::dpi) module for more information.
    ///
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Can be overridden using the `WINIT_X11_SCALE_FACTOR` environment variable.
    #[inline]
    pub fn scale_factor(&self) -> Result<f64, MonitorGone> {
        self.inner.scale_factor()
    }

    /// Returns all fullscreen video modes supported by this monitor.
    #[inline]
    pub fn video_modes(&self) -> Result<impl Iterator<Item = VideoMode>, MonitorGone> {
        self.inner
            .video_modes()
            .map(|modes| modes.map(|video_mode| VideoMode { video_mode }))
    }
}
