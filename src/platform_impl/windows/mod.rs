#![cfg(target_os = "windows")]

use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    fmt,
    hash::{Hash, Hasher},
    ptr,
};

use winapi::{
    self,
    shared::windef::{HMENU, HWND},
    um::winnt::HANDLE,
};

pub use self::{
    event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget},
    gamepad::GamepadShared,
    icon::WinIcon,
    monitor::{MonitorHandle, VideoMode},
    window::Window,
};

pub use self::icon::WinIcon as PlatformIcon;

use crate::event::device;
use crate::icon::Icon;
use crate::window::Theme;

#[derive(Clone)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub parent: Option<HWND>,
    pub menu: Option<HMENU>,
    pub taskbar_icon: Option<Icon>,
    pub no_redirection_bitmap: bool,
    pub drag_and_drop: bool,
    pub preferred_theme: Option<Theme>,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> Self {
        Self {
            parent: None,
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

macro_rules! device_id {
    ($name:ident, $enumerate:ident) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) struct $name(HANDLE);

        unsafe impl Send for $name {}
        unsafe impl Sync for $name {}

        impl $name {
            pub unsafe fn dummy() -> Self {
                Self(ptr::null_mut())
            }

            pub fn persistent_identifier(&self) -> Option<String> {
                raw_input::get_raw_input_device_name(self.0)
            }

            pub fn is_connected(&self) -> bool {
                raw_input::get_raw_input_device_info(self.0).is_some()
            }

            #[inline(always)]
            pub fn handle(&self) -> HANDLE {
                self.0
            }

            pub fn enumerate<'a, T>(
                event_loop: &'a EventLoop<T>,
            ) -> impl 'a + Iterator<Item = device::$name> {
                event_loop.$enumerate()
            }
        }

        impl From<$name> for device::$name {
            fn from(platform_id: $name) -> Self {
                Self(platform_id)
            }
        }
    };
}

device_id!(MouseId, mouses);
device_id!(KeyboardId, keyboards);
device_id!(HidId, hids);

#[derive(Clone)]
pub(crate) struct GamepadHandle {
    handle: HANDLE,
    shared_data: GamepadShared,
}

unsafe impl Send for GamepadHandle where GamepadShared: Send {}
unsafe impl Sync for GamepadHandle where GamepadShared: Sync {}

impl GamepadHandle {
    pub unsafe fn dummy() -> Self {
        Self {
            handle: ptr::null_mut(),
            shared_data: GamepadShared::Dummy,
        }
    }

    pub fn persistent_identifier(&self) -> Option<String> {
        raw_input::get_raw_input_device_name(self.handle)
    }

    pub fn is_connected(&self) -> bool {
        raw_input::get_raw_input_device_info(self.handle).is_some()
    }

    #[inline(always)]
    pub fn handle(&self) -> HANDLE {
        self.handle
    }

    pub fn rumble(&self, left_speed: f64, right_speed: f64) -> Result<(), device::RumbleError> {
        self.shared_data.rumble(left_speed, right_speed)
    }

    pub fn port(&self) -> Option<u8> {
        self.shared_data.port()
    }

    pub fn battery_level(&self) -> Option<device::BatteryLevel> {
        self.shared_data.battery_level()
    }

    pub fn enumerate<'a, T>(
        event_loop: &'a EventLoop<T>,
    ) -> impl 'a + Iterator<Item = device::GamepadHandle> {
        event_loop.gamepads()
    }
}

impl From<GamepadHandle> for device::GamepadHandle {
    fn from(platform_id: GamepadHandle) -> Self {
        Self(platform_id)
    }
}

impl fmt::Debug for GamepadHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_tuple("GamepadHandle").field(&self.handle).finish()
    }
}

impl Eq for GamepadHandle {}
impl PartialEq for GamepadHandle {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl Ord for GamepadHandle {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        self.handle.cmp(&other.handle)
    }
}
impl PartialOrd for GamepadHandle {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.handle.partial_cmp(&other.handle)
    }
}

impl Hash for GamepadHandle {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.handle.hash(state);
    }
}

#[macro_use]
mod util;
mod dark_mode;
mod dpi;
mod drop_handler;
mod event;
mod event_loop;
mod gamepad;
mod icon;
mod monitor;
mod raw_input;
mod window;
mod window_state;
mod xinput;
