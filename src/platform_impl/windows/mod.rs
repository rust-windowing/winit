use smol_str::SmolStr;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{HMENU, WINDOW_LONG_PTR_INDEX};

pub(crate) use self::event_loop::{
    EventLoop, EventLoopProxy, OwnedDisplayHandle, PlatformSpecificEventLoopAttributes,
};
pub use self::icon::WinIcon as PlatformIcon;
pub(crate) use self::icon::{SelectedCursor, WinCursor as PlatformCustomCursor, WinIcon};
pub(crate) use self::keyboard::{physicalkey_to_scancode, scancode_to_physicalkey};
pub(crate) use self::monitor::{MonitorHandle, VideoModeHandle};
pub(crate) use self::window::Window;
pub(crate) use crate::cursor::OnlyCursorImageSource as PlatformCustomCursorSource;
use crate::event::DeviceId;
use crate::icon::Icon;
use crate::keyboard::Key;
use crate::platform::windows::{BackdropType, Color, CornerPreference};
use crate::platform_impl::Fullscreen;

#[derive(Clone, Debug, PartialEq)]
pub struct PlatformSpecificWindowAttributes {
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

impl Default for PlatformSpecificWindowAttributes {
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

unsafe impl Send for PlatformSpecificWindowAttributes {}
unsafe impl Sync for PlatformSpecificWindowAttributes {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FingerId {
    id: u32,
}

impl FingerId {
    #[cfg(test)]
    pub const fn dummy() -> Self {
        FingerId { id: 0 }
    }
}

fn wrap_device_id(id: u32) -> DeviceId {
    DeviceId::from_raw(id as i64)
}

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
