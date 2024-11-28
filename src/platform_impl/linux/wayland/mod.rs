//! Winit's Wayland backend.

pub use event_loop::{ActiveEventLoop, EventLoop};
pub use output::{MonitorHandle, VideoModeHandle};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Proxy;
pub use window::Window;

pub(super) use crate::cursor::OnlyCursorImage as CustomCursor;
use crate::dpi::{LogicalSize, PhysicalSize};
use crate::window::WindowId;

mod event_loop;
mod output;
mod seat;
mod state;
mod types;
mod window;

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

mod xdg_session_management {
    #![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
    #![allow(non_upper_case_globals, non_snake_case, unused_imports)]
    #![allow(missing_docs, clippy::all)]

    use wayland_client;
    use wayland_protocols::xdg::shell::client::*;

    pub mod __interfaces {
        use wayland_protocols::xdg::shell::client::__interfaces::*;
        wayland_scanner::generate_interfaces!(
            "./src/platform_impl/linux/wayland/session-management-v1.xml"
        );
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!(
        "./src/platform_impl/linux/wayland/session-management-v1.xml"
    );
}
