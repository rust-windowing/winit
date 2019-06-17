//! Types useful for interacting with a user's monitors.
//!
//! If you want to get basic information about a monitor, you can use the [`MonitorHandle`][monitor_id]
//! type. This is retreived from an [`AvailableMonitorsIter`][monitor_iter], which can be acquired
//! with:
//! - [`EventLoop::available_monitors`][loop_get]
//! - [`Window::available_monitors`][window_get].
//!
//! [monitor_id]: ./struct.MonitorHandle.html
//! [monitor_iter]: ./struct.AvailableMonitorsIter.html
//! [loop_get]: ../event_loop/struct.EventLoop.html#method.available_monitors
//! [window_get]: ../window/struct.Window.html#method.available_monitors
use std::collections::vec_deque::IntoIter as VecDequeIter;

use crate::platform_impl;
use crate::dpi::{PhysicalPosition, PhysicalSize};

/// An iterator over all available monitors.
///
/// Can be acquired with:
/// - [`EventLoop::available_monitors`][loop_get]
/// - [`Window::available_monitors`][window_get].
///
/// [loop_get]: ../event_loop/struct.EventLoop.html#method.available_monitors
/// [window_get]: ../window/struct.Window.html#method.available_monitors
// Implementation note: we retrieve the list once, then serve each element by one by one.
// This may change in the future.
#[derive(Debug)]
pub struct AvailableMonitorsIter {
    pub(crate) data: VecDequeIter<platform_impl::MonitorHandle>,
}

impl Iterator for AvailableMonitorsIter {
    type Item = MonitorHandle;

    #[inline]
    fn next(&mut self) -> Option<MonitorHandle> {
        self.data.next().map(|id| MonitorHandle { inner: id })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.data.size_hint()
    }
}

/// Describes a fullscreen video mode of a monitor.
///
/// Can be acquired with:
/// - [`MonitorHandle::video_modes`][monitor_get].
///
/// [monitor_get]: ../monitor/struct.MonitorHandle.html#method.video_modes
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct VideoMode {
    pub(crate) size: (u32, u32),
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate: u16,
}

impl VideoMode {
    /// Returns the resolution of this video mode.
    pub fn size(&self) -> PhysicalSize {
        self.size.into()
    }

    /// Returns the bit depth of this video mode, as in how many bits you have
    /// available per color. This is generally 24 bits or 32 bits on modern
    /// systems, depending on whether the alpha channel is counted or not.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland:** Always returns 32.
    /// - **iOS:** Always returns 32.
    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    /// Returns the refresh rate of this video mode. **Note**: the returned
    /// refresh rate is an integer approximation, and you shouldn't rely on this
    /// value to be exact.
    pub fn refresh_rate(&self) -> u16 {
        self.refresh_rate
    }
}

/// Handle to a monitor.
///
/// Allows you to retrieve information about a given monitor and can be used in [`Window`] creation.
///
/// [`Window`]: ../window/struct.Window.html
#[derive(Debug, Clone)]
pub struct MonitorHandle {
    pub(crate) inner: platform_impl::MonitorHandle
}

impl MonitorHandle {
    /// Returns a human-readable name of the monitor.
    ///
    /// Returns `None` if the monitor doesn't exist anymore.
    #[inline]
    pub fn name(&self) -> Option<String> {
        self.inner.name()
    }

    /// Returns the monitor's resolution.
    #[inline]
    pub fn size(&self) -> PhysicalSize {
        self.inner.size()
    }

    /// Returns the top-left corner position of the monitor relative to the larger full
    /// screen area.
    #[inline]
    pub fn position(&self) -> PhysicalPosition {
        self.inner.position()
    }

    /// Returns the DPI factor that can be used to map logical pixels to physical pixels, and vice versa.
    ///
    /// See the [`dpi`](dpi/index.html) module for more information.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Can be overridden using the `WINIT_HIDPI_FACTOR` environment variable.
    /// - **Android:** Always returns 1.0.
    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        self.inner.hidpi_factor()
    }

    /// Returns all fullscreen video modes supported by this monitor.
    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        self.inner.video_modes()
    }
}
