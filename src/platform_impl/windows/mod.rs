#![cfg(target_os = "windows")]

use winapi::{self, shared::windef::HMENU, shared::windef::HWND};

pub use self::{
    event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget},
    icon::WinIcon,
    monitor::{MonitorHandle, VideoMode},
    window::Window,
};

pub use self::icon::WinIcon as PlatformIcon;

use crate::event::DeviceId as RootDeviceId;
use crate::icon::Icon;
use crate::window::Theme;

#[derive(Clone)]
pub enum Parent {
    None,
    ChildOf(HWND),
    OwnedBy(HWND),
}

#[derive(Clone)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub parent: Parent,
    pub menu: Option<HMENU>,
    pub taskbar_icon: Option<Icon>,
    pub no_redirection_bitmap: bool,
    pub drag_and_drop: bool,
    pub preferred_theme: Option<Theme>,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> Self {
        Self {
            parent: Parent::None,
            menu: None,
            taskbar_icon: None,
            no_redirection_bitmap: false,
            drag_and_drop: true,
            preferred_theme: None,
        }
    }
}

unsafe impl Send for PlatformSpecificWindowBuilderAttributes {}
unsafe impl Sync for PlatformSpecificWindowBuilderAttributes {}

// Cursor name in UTF-16. Used to set cursor in `WM_SETCURSOR`.
#[derive(Debug, Clone, Copy)]
pub struct Cursor(pub *const winapi::ctypes::wchar_t);
unsafe impl Send for Cursor {}
unsafe impl Sync for Cursor {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(u32);

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        DeviceId(0)
    }
}

impl DeviceId {
    pub fn persistent_identifier(&self) -> Option<String> {
        if self.0 != 0 {
            raw_input::get_raw_input_device_name(self.0 as _)
        } else {
            None
        }
    }
}

// Constant device ID, to be removed when this backend is updated to report real device IDs.
const DEVICE_ID: RootDeviceId = RootDeviceId(DeviceId(0));

fn wrap_device_id(id: u32) -> RootDeviceId {
    RootDeviceId(DeviceId(id))
}

pub type OsError = std::io::Error;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(HWND);
unsafe impl Send for WindowId {}
unsafe impl Sync for WindowId {}

impl WindowId {
    pub unsafe fn dummy() -> Self {
        use std::ptr::null_mut;

        WindowId(null_mut())
    }
}

#[macro_use]
mod util;
mod dark_mode;
mod dpi;
mod drop_handler;
mod event;
mod event_loop;
mod icon;
mod monitor;
mod raw_input;
mod window;
mod window_state;
