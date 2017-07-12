#![cfg(target_os = "windows")]

use winapi;

pub use self::events_loop::{EventsLoop, EventsLoopProxy};
pub use self::monitor::{MonitorId, get_available_monitors, get_primary_monitor};
pub use self::window::Window;

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub parent: Option<winapi::HWND>,
}

unsafe impl Send for PlatformSpecificWindowBuilderAttributes {}
unsafe impl Sync for PlatformSpecificWindowBuilderAttributes {}

// TODO: document what this means
pub type Cursor = *const winapi::wchar_t;

// TODO: remove
pub type Window2 = Window;

// Constant device ID, to be removed when this backend is updated to report real device IDs.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;
const DEVICE_ID: ::DeviceId = ::DeviceId(DeviceId);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(winapi::HWND);
unsafe impl Send for WindowId {}
unsafe impl Sync for WindowId {}

mod event;
mod events_loop;
mod monitor;
mod window;
