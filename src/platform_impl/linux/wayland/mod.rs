//! Winit's Wayland backend.

use dpi::{LogicalSize, PhysicalSize};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Proxy;
use winit_core::window::WindowId;

mod event_loop;
mod output;
mod seat;
mod state;
mod types;
mod window;

pub use event_loop::{ActiveEventLoop, EventLoop};
pub use window::Window;

/// Get the WindowId out of the surface.
#[inline]
fn make_wid(surface: &WlSurface) -> WindowId {
    WindowId::from_raw(surface.id().as_ptr() as usize)
}

/// The default routine does floor, but we need round on Wayland.
fn logical_to_physical_rounded(size: LogicalSize<u32>, scale_factor: f64) -> PhysicalSize<u32> {
    let width = size.width as f64 * scale_factor;
    let height = size.height as f64 * scale_factor;
    (width.round(), height.round()).into()
}
