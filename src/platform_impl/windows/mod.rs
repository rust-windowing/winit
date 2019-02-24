#![cfg(target_os = "windows")]

mod dpi;
mod drop_handler;
mod event;
mod event_loop;
mod gamepad;
mod icon;
mod monitor;
mod raw_input;
mod util;
mod window;
mod window_state;
mod xinput;

use std::ptr;
use winapi;
use winapi::shared::windef::HWND;
use winapi::um::winnt::HANDLE;
use window::Icon;

pub use self::event_loop::{EventLoop, EventLoopWindowTarget, EventLoopProxy};
pub use self::monitor::MonitorHandle;
pub use self::window::Window;

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub parent: Option<HWND>,
    pub taskbar_icon: Option<Icon>,
    pub no_redirection_bitmap: bool,
}

unsafe impl Send for PlatformSpecificWindowBuilderAttributes {}
unsafe impl Sync for PlatformSpecificWindowBuilderAttributes {}

// Cursor name in UTF-16. Used to set cursor in `WM_SETCURSOR`.
#[derive(Debug, Clone, Copy)]
pub struct Cursor(pub *const winapi::ctypes::wchar_t);
unsafe impl Send for Cursor {}
unsafe impl Sync for Cursor {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(HWND);
unsafe impl Send for WindowId {}
unsafe impl Sync for WindowId {}

impl WindowId {
    pub unsafe fn dummy() -> Self {
        WindowId(ptr::null_mut())
    }
}

macro_rules! device_id {
    ($name:ident) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) struct $name(HANDLE);

        impl $name {
            pub unsafe fn dummy() -> Self {
                Self(ptr::null_mut())
            }

            pub fn get_persistent_identifier(&self) -> Option<String> {
                raw_input::get_raw_input_device_name(self.0)
            }
        }

        impl From<$name> for crate::event::device::$name {
            fn from(platform_id: $name) -> Self {
                Self(platform_id)
            }
        }
    }
}

device_id!(MouseId);
device_id!(KeyboardId);
device_id!(GamepadHandle);
