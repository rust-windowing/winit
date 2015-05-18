#![cfg(all(target_os = "linux", feature = "window"))]

pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};
pub use self::window::{Window, XWindow, PollEventsIterator, WaitEventsIterator, Context, WindowProxy};
pub use self::xdisplay::XConnection;

pub mod ffi;

mod events;
mod monitor;
mod window;
mod xdisplay;
