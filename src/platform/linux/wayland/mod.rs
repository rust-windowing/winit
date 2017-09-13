#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

pub use self::window::{Window, WindowId};
pub use self::event_loop::{EventsLoop, EventsLoopProxy};
pub use self::context::{WaylandContext, MonitorId, get_available_monitors,
                        get_primary_monitor};

use self::window::{make_wid, DecoratedHandler};
use self::event_loop::EventsLoopSink;

extern crate wayland_kbd;
extern crate wayland_window;
extern crate wayland_protocols;
extern crate tempfile;

mod context;
mod event_loop;
mod keyboard;
mod window;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;
