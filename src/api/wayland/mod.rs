#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

pub use self::window::{PollEventsIterator, WaitEventsIterator, Window, WindowProxy};
pub use self::context::{WaylandContext,MonitorId};

extern crate wayland_kbd;
extern crate wayland_window;

mod context;
mod window;
