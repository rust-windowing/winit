#![cfg(target_os = "windows")]

use windows_sys::Win32::{
    Foundation::{HANDLE, HWND},
    UI::WindowsAndMessaging::{HMENU, WINDOW_LONG_PTR_INDEX},
};

pub(crate) use self::{
    event_loop::{
        EventLoop, EventLoopProxy, EventLoopWindowTarget, PlatformSpecificEventLoopAttributes,
    },
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
    pub skip_taskbar: bool,
    pub decoration_shadow: bool,
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
            skip_taskbar: false,
            decoration_shadow: false,
        }
    }
}

unsafe impl Send for PlatformSpecificWindowBuilderAttributes {}
unsafe impl Sync for PlatformSpecificWindowBuilderAttributes {}

// Cursor name in UTF-16. Used to set cursor in `WM_SETCURSOR`.
#[derive(Debug, Clone, Copy)]
pub struct Cursor(pub *const u16);
unsafe impl Send for Cursor {}
unsafe impl Sync for Cursor {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(u32);

impl DeviceId {
    pub const unsafe fn dummy() -> Self {
        DeviceId(0)
    }
}

impl DeviceId {
    pub fn persistent_identifier(&self) -> Option<String> {
        if self.0 != 0 {
            raw_input::get_raw_input_device_name(self.0 as HANDLE)
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
    pub const unsafe fn dummy() -> Self {
        WindowId(0)
    }
}

impl From<WindowId> for u64 {
    fn from(window_id: WindowId) -> Self {
        window_id.0 as u64
    }
}

impl From<WindowId> for HWND {
    fn from(window_id: WindowId) -> Self {
        window_id.0
    }
}

impl From<u64> for WindowId {
    fn from(raw_id: u64) -> Self {
        Self(raw_id as HWND)
    }
}

#[inline(always)]
const fn get_xbutton_wparam(x: u32) -> u16 {
    loword(x)
}

#[inline(always)]
const fn get_x_lparam(x: u32) -> i16 {
    loword(x) as _
}

#[inline(always)]
const fn get_y_lparam(x: u32) -> i16 {
    hiword(x) as _
}

#[inline(always)]
const fn loword(x: u32) -> u16 {
    (x & 0xFFFF) as u16
}

#[inline(always)]
const fn hiword(x: u32) -> u16 {
    ((x >> 16) & 0xFFFF) as u16
}

#[inline(always)]
unsafe fn get_window_long(hwnd: HWND, nindex: WINDOW_LONG_PTR_INDEX) -> isize {
    #[cfg(target_pointer_width = "64")]
    return windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, nindex);
    #[cfg(target_pointer_width = "32")]
    return windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW(hwnd, nindex) as isize;
}

#[inline(always)]
unsafe fn set_window_long(hwnd: HWND, nindex: WINDOW_LONG_PTR_INDEX, dwnewlong: isize) -> isize {
    #[cfg(target_pointer_width = "64")]
    return windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(hwnd, nindex, dwnewlong);
    #[cfg(target_pointer_width = "32")]
    return windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongW(
        hwnd,
        nindex,
        dwnewlong as i32,
    ) as isize;
}

#[macro_use]
mod util;
mod dark_mode;
mod definitions;
mod dpi;
mod drop_handler;
mod event;
mod event_loop;
mod icon;
mod ime;
mod monitor;
mod raw_input;
mod window;
mod window_state;
