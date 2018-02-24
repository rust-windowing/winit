#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

pub use self::window::Window;
pub use self::event_loop::{EventsLoop, EventsLoopProxy, EventsLoopSink, MonitorId};

extern crate wayland_kbd;
extern crate wayland_window;
extern crate wayland_protocols;

use wayland_client::protocol::wl_surface;
use wayland_client::Proxy;

mod event_loop;
mod pointer;
mod touch;
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