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

use std::cmp::{Ordering, Eq, Ord, PartialEq, PartialOrd};
use std::hash::{Hash, Hasher};
use std::fmt;
use std::ptr;
use winapi;
use winapi::shared::windef::HWND;
use winapi::um::winnt::HANDLE;
use window::Icon;

pub use self::event_loop::{EventLoop, EventLoopWindowTarget, EventLoopProxy};
pub use self::gamepad::GamepadRumbler;
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

        unsafe impl Send for $name {}
        unsafe impl Sync for $name {}

        impl $name {
            pub unsafe fn dummy() -> Self {
                Self(ptr::null_mut())
            }

            pub fn get_persistent_identifier(&self) -> Option<String> {
                raw_input::get_raw_input_device_name(self.0)
            }

            #[inline(always)]
            pub fn handle(&self) -> HANDLE {
                self.0
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

#[derive(Clone)]
pub(crate) struct GamepadHandle {
    handle: HANDLE,
    rumbler: GamepadRumbler,
}

unsafe impl Send for GamepadHandle where GamepadRumbler: Send {}
unsafe impl Sync for GamepadHandle where GamepadRumbler: Sync {}

impl GamepadHandle {
    pub unsafe fn dummy() -> Self {
        Self {
            handle: ptr::null_mut(),
            rumbler: GamepadRumbler::Dummy,
        }
    }

    pub fn get_persistent_identifier(&self) -> Option<String> {
        raw_input::get_raw_input_device_name(self.handle)
    }

    #[inline(always)]
    pub fn handle(&self) -> HANDLE {
        self.handle
    }

    pub fn rumble(&self, left_speed: f64, right_speed: f64) {
        self.rumbler.rumble(left_speed, right_speed);
    }
}

impl From<GamepadHandle> for crate::event::device::GamepadHandle {
    fn from(platform_id: GamepadHandle) -> Self {
        Self(platform_id)
    }
}

impl fmt::Debug for GamepadHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_tuple("GamepadHandle")
            .field(&self.handle)
            .finish()
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
