//! Types useful for interacting with a user's monitors.
//!
//! If you want to get basic information about a monitor, you can use the [`MonitorHandle`][monitor_id]
//! type. This is retreived from an [`AvailableMonitorsIter`][monitor_iter], which can be acquired
//! with:
//! - [`EventLoop::get_available_monitors`][loop_get]
//! - [`Window::get_available_monitors`][window_get].
//!
//! [monitor_id]: ./struct.MonitorHandle.html
//! [monitor_iter]: ./struct.AvailableMonitorsIter.html
//! [loop_get]: ../event_loop/struct.EventLoop.html#method.get_available_monitors
//! [window_get]: ../window/struct.Window.html#method.get_available_monitors
use std::collections::vec_deque::IntoIter as VecDequeIter;

use platform_impl;
use dpi::{PhysicalPosition, PhysicalSize};

/// An iterator over all available monitors.
///
/// Can be acquired with:
/// - [`EventLoop::get_available_monitors`][loop_get]
/// - [`Window::get_available_monitors`][window_get].
///
/// [loop_get]: ../event_loop/struct.EventLoop.html#method.get_available_monitors
/// [window_get]: ../window/struct.Window.html#method.get_available_monitors
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
    pub fn get_name(&self) -> Option<String> {
        self.inner.get_name()
    }

    /// Returns the monitor's resolution.
    #[inline]
    pub fn get_dimensions(&self) -> PhysicalSize {
        self.inner.get_dimensions()
    }

    /// Returns the top-left corner position of the monitor relative to the larger full
    /// screen area.
    #[inline]
    pub fn get_position(&self) -> PhysicalPosition {
        self.inner.get_position()
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
    pub fn get_hidpi_factor(&self) -> f64 {
        self.inner.get_hidpi_factor()
    }
}
