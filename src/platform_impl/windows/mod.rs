#![cfg(target_os = "windows")]

use winapi::{self, shared::windef::HWND, um::winnt::HANDLE};

pub use self::{
    event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget},
    icon::WinIcon,
    monitor::{MonitorHandle, VideoMode},
    window::Window,
};

pub use self::icon::WinIcon as PlatformIcon;

use crate::icon::Icon;

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

macro_rules! device_id {
    ($name:ident) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) struct $name(HANDLE);

        unsafe impl Send for $name {}
        unsafe impl Sync for $name {}

        impl $name {
            pub unsafe fn dummy() -> Self {
                Self(std::ptr::null_mut())
            }

            pub fn persistent_identifier(&self) -> Option<String> {
                raw_input::get_raw_input_device_name(self.0)
            }

            #[inline(always)]
            pub fn handle(&self) -> HANDLE {
                self.0
            }
        }

        impl From<$name> for crate::event::$name {
            fn from(platform_id: $name) -> Self {
                Self(platform_id)
            }
        }
    };
}

device_id!(PointerDeviceId);
device_id!(KeyboardDeviceId);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MouseId;
impl MouseId {
    pub unsafe fn dummy() -> Self {
        MouseId
    }
}
impl From<MouseId> for crate::event::MouseId {
    fn from(platform_id: MouseId) -> Self {
        Self(platform_id)
    }
}

impl crate::event::PointerId {
    const MOUSE_ID: Self = Self::MouseId(crate::event::MouseId(MouseId));
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TouchId(u32);
impl TouchId {
    pub unsafe fn dummy() -> Self {
        TouchId(!0)
    }
}
impl From<TouchId> for crate::event::TouchId {
    fn from(platform_id: TouchId) -> Self {
        Self(platform_id)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PenId(u32);
impl PenId {
    pub unsafe fn dummy() -> Self {
        PenId(!0)
    }
}
impl From<PenId> for crate::event::PenId {
    fn from(platform_id: PenId) -> Self {
        Self(platform_id)
    }
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

impl From<WindowId> for crate::window::WindowId {
    fn from(platform_id: WindowId) -> Self {
        Self(platform_id)
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
