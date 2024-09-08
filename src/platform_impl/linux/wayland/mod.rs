//! Winit's Wayland backend.

pub use event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
pub use output::{MonitorHandle, VideoModeHandle};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Proxy;
pub use window::Window;

pub(super) use crate::cursor::OnlyCursorImage as CustomCursor;
use crate::dpi::{LogicalSize, PhysicalSize};
pub use crate::platform_impl::platform::WindowId;

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
    pub const fn dummy() -> Self {
        DeviceId
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FingerId(i32);

impl FingerId {
    pub const fn dummy() -> Self {
        FingerId(0)
    }
}

/// Get the WindowId out of the surface.
#[inline]
fn make_wid(surface: &WlSurface) -> WindowId {
    WindowId(surface.id().as_ptr() as u64)
}

/// The default routine does floor, but we need round on Wayland.
fn logical_to_physical_rounded(size: LogicalSize<u32>, scale_factor: f64) -> PhysicalSize<u32> {
    let width = size.width as f64 * scale_factor;
    let height = size.height as f64 * scale_factor;
    (width.round(), height.round()).into()
}
