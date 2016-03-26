#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

pub use self::monitor::{MonitorId, get_available_monitors, get_primary_monitor};
pub use self::window::{Window, XWindow, PollEventsIterator, WaitEventsIterator, WindowProxy};
pub use self::xdisplay::{XConnection, XNotSupported, XError};

pub mod ffi;

mod events;
mod input;
mod monitor;
mod window;
mod xdisplay;
