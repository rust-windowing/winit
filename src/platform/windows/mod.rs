#![cfg(target_os = "windows")]

use winapi;
use winapi::shared::windef::HWND;

pub use self::events_loop::{EventsLoop, EventsLoopProxy};
pub use self::monitor::MonitorId;
pub use self::window::Window;

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub parent: Option<HWND>,
}

unsafe impl Send for PlatformSpecificWindowBuilderAttributes {}
unsafe impl Sync for PlatformSpecificWindowBuilderAttributes {}

// TODO: document what this means
pub type Cursor = *const winapi::ctypes::wchar_t;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(u32);

impl DeviceId {
    pub fn get_persistent_identifier(&self) -> Option<String> {
        if self.0 != 0 {
            raw_input::get_raw_input_device_name(self.0 as _)
        } else {
            None
        }
    }
}

// Constant device ID, to be removed when this backend is updated to report real device IDs.
const DEVICE_ID: ::DeviceId = ::DeviceId(DeviceId(0));

fn wrap_device_id(id: u32) -> ::DeviceId {
    ::DeviceId(DeviceId(id))
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(HWND);
unsafe impl Send for WindowId {}
unsafe impl Sync for WindowId {}

mod event;
mod events_loop;
mod monitor;
mod raw_input;
mod util;
mod window;
