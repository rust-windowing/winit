use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX;

pub(crate) use self::event_loop::{EventLoop, PlatformSpecificEventLoopAttributes};
pub(crate) use self::icon::{RaiiIcon, SelectedCursor};
pub(crate) use self::keyboard::{physicalkey_to_scancode, scancode_to_physicalkey};
pub(crate) use self::monitor::MonitorHandle;
pub(crate) use self::window::Window;
use crate::event::DeviceId;

fn wrap_device_id(id: u32) -> DeviceId {
    DeviceId::from_raw(id as i64)
}

#[inline(always)]
const fn get_xbutton_wparam(x: u32) -> u16 {
    hiword(x)
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
pub(crate) const fn primarylangid(lgid: u16) -> u16 {
    lgid & 0x3ff
}

#[inline(always)]
pub(crate) const fn loword(x: u32) -> u16 {
    (x & 0xffff) as u16
}

#[inline(always)]
const fn hiword(x: u32) -> u16 {
    ((x >> 16) & 0xffff) as u16
}

#[inline(always)]
unsafe fn get_window_long(hwnd: HWND, nindex: WINDOW_LONG_PTR_INDEX) -> isize {
    #[cfg(target_pointer_width = "64")]
    return unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, nindex) };
    #[cfg(target_pointer_width = "32")]
    return unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW(hwnd, nindex) as isize
    };
}

#[inline(always)]
unsafe fn set_window_long(hwnd: HWND, nindex: WINDOW_LONG_PTR_INDEX, dwnewlong: isize) -> isize {
    #[cfg(target_pointer_width = "64")]
    return unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(hwnd, nindex, dwnewlong)
    };
    #[cfg(target_pointer_width = "32")]
    return unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongW(hwnd, nindex, dwnewlong as i32)
            as isize
    };
}

#[macro_use]
mod util;
mod dark_mode;
mod definitions;
mod dpi;
mod drop_handler;
mod event_loop;
mod icon;
mod ime;
mod keyboard;
mod keyboard_layout;
mod monitor;
pub(crate) mod raw_input;
mod window;
mod window_state;
