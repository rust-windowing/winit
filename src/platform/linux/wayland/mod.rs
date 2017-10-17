#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

pub use self::window::Window;
pub use self::event_loop::{EventsLoop, EventsLoopProxy, EventsLoopSink};
pub use self::context::{WaylandContext, MonitorId, get_available_monitors,
                        get_primary_monitor};

extern crate wayland_kbd;
extern crate wayland_window;
extern crate wayland_protocols;
extern crate tempfile;

use wayland_client::protocol::wl_surface;
use wayland_client::Proxy;

mod context;
mod event_loop;
mod keyboard;
mod window;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(usize);

#[inline]
fn make_wid(s: &wl_surface::WlSurface) -> WindowId {
    WindowId(s.ptr() as usize)
}