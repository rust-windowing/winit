#![cfg(all(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"), feature = "window"))]

pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};
pub use self::window::{Window, XWindow, PollEventsIterator, WaitEventsIterator, Context, WindowProxy};
pub use self::xdisplay::XConnection;

pub mod ffi;

mod events;
mod input;
mod monitor;
mod window;
mod xdisplay;
