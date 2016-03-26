#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

pub use self::monitor::{MonitorId, get_available_monitors, get_primary_monitor};
pub use self::window::{PollEventsIterator, WaitEventsIterator, Window, WindowProxy};

extern crate wayland_kbd;
extern crate wayland_window;

mod context;
mod events;
mod keyboard;
mod monitor;
mod window;

#[inline]
pub fn is_available() -> bool {
    context::WAYLAND_CONTEXT.is_some()
}
