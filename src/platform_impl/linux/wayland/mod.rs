#![cfg(wayland_platform)]

//! Winit's Wayland backend.

use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Proxy;

pub use crate::platform_impl::platform::WindowId;
pub use event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget};
pub use output::{MonitorHandle, VideoMode};
pub use window::Window;

mod event_loop;
mod output;
mod seat;
mod state;
mod types;
mod window;

/// Dummy device id, since Wayland doesn't have device events.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

impl DeviceId {
    pub const unsafe fn dummy() -> Self {
        DeviceId
    }
}

/// Get the WindowId out of the surface.
#[inline]
fn make_wid(surface: &WlSurface) -> WindowId {
    WindowId(surface.id().as_ptr() as u64)
}
