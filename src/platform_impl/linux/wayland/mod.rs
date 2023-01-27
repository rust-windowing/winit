#![cfg(wayland_platform)]

use sctk::reexports::client::protocol::wl_surface::WlSurface;

pub use crate::platform_impl::platform::WindowId;
pub use event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget};
pub use output::{MonitorHandle, VideoMode};
pub use window::Window;

mod env;
mod event_loop;
mod output;
mod protocols;
mod seat;
mod window;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

impl DeviceId {
    pub const unsafe fn dummy() -> Self {
        DeviceId
    }
}

#[inline]
fn make_wid(surface: &WlSurface) -> WindowId {
    WindowId(surface.as_ref().c_ptr() as u64)
}
