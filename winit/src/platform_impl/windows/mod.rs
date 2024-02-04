#![cfg(windows_platform)]

use smol_str::SmolStr;
use windows_sys::Win32::{
    Foundation::{HANDLE, HWND},
    UI::WindowsAndMessaging::{HMENU, WINDOW_LONG_PTR_INDEX},
};

pub(crate) use self::{
    event_loop::{
        EventLoop, EventLoopProxy, EventLoopWindowTarget, OwnedDisplayHandle,
        PlatformSpecificEventLoopAttributes,
    },
    icon::{SelectedCursor, WinIcon},
    keyboard::{physicalkey_to_scancode, scancode_to_physicalkey},
    monitor::{MonitorHandle, VideoModeHandle},
    window::Window,
};

pub(crate) use self::icon::WinCursor as PlatformCustomCursor;
pub use self::icon::WinIcon as PlatformIcon;
pub(crate) use crate::cursor::OnlyCursorImageBuilder as PlatformCustomCursorBuilder;
use crate::platform_impl::Fullscreen;

use crate::event::DeviceId;
use crate::icon::Icon;
use crate::keyboard::Key;
use crate::platform::windows::{BackdropType, Color, CornerPreference};

#[derive(Clone, Debug)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub owner: Option<HWND>,
    pub menu: Option<HMENU>,
    pub taskbar_icon: Option<Icon>,
    pub no_redirection_bitmap: bool,
    pub drag_and_drop: bool,
    pub skip_taskbar: bool,
    pub class_name: String,
    pub decoration_shadow: bool,
    pub backdrop_type: BackdropType,
    pub clip_children: bool,
    pub border_color: Option<Color>,
    pub title_background_color: Option<Color>,
    pub title_text_color: Option<Color>,
    pub corner_preference: Option<CornerPreference>,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> Self {
        Self {
            owner: None,
            menu: None,
            taskbar_icon: None,
            no_redirection_bitmap: false,
            drag_and_drop: true,
            skip_taskbar: false,
            class_name: "Window Class".to_string(),
            decoration_shadow: false,
            backdrop_type: BackdropType::default(),
            clip_children: true,
            border_color: None,
            title_background_color: None,
            title_text_color: None,
            corner_preference: None,
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

    pub fn persistent_identifier(id: DeviceId) -> Option<String> {
        let val: u64 = id.into();
        if val != 0 {
            raw_input::get_raw_input_device_name(val as HANDLE)
        } else {
            None
        }
    }

// Constant device ID, to be removed when this backend is updated to report real device IDs.
const DEVICE_ID: DeviceId = unsafe { DeviceId::dummy() };

fn wrap_device_id(id: u32) -> DeviceId {
    (id as u64).into()
}

pub type OsError = std::io::Error;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct KeyEventExtra {
    pub text_with_all_modifiers: Option<SmolStr>,
    pub key_without_modifiers: Key,
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
    lgid & 0x3FF
}

#[inline(always)]
pub(crate) const fn loword(x: u32) -> u16 {
    (x & 0xFFFF) as u16
}

#[inline(always)]
const fn hiword(x: u32) -> u16 {
    ((x >> 16) & 0xFFFF) as u16
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
mod raw_input;
mod window;
mod window_state;
