use super::event_loop::EventLoop;
use crate::event::device;
use crate::platform_impl::platform::backend;

use std::{
    cmp::{Eq, Ordering, PartialEq, PartialOrd},
    fmt,
    hash::{Hash, Hasher},
};

macro_rules! device_id {
    ($name:ident, $enumerate:ident) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) struct $name(pub i32);

        unsafe impl Send for $name {}
        unsafe impl Sync for $name {}

        impl $name {
            pub unsafe fn dummy() -> Self {
                Self(0)
            }

            pub fn is_connected(&self) -> bool {
                false
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
    pub(crate) index: i32,
    pub(crate) manager: backend::GamepadManagerShared,
}

impl GamepadHandle {
    pub unsafe fn dummy() -> Self {
        Self {
            index: -1,
            manager: backend::GamepadManagerShared::default(),
        }
    }

    fn gamepad(&self) -> backend::Gamepad {
        self.manager
            .get(&(self.index as u32))
            .unwrap_or(backend::Gamepad::default())
    }

    fn is_dummy(&self) -> bool {
        self.manager.is_present(&(self.index as u32))
    }

    pub fn is_connected(&self) -> bool {
        self.gamepad().connected()
    }

    pub fn enumerate<'a, T>(
        event_loop: &'a EventLoop<T>,
    ) -> impl 'a + Iterator<Item = device::GamepadHandle> {
        event_loop.gamepads()
    }

    pub fn rumble(&self, _left_speed: f64, _right_speed: f64) -> Result<(), device::RumbleError> {
        Ok(())
    }

    pub fn port(&self) -> Option<u8> {
        None
    }

    pub fn battery_level(&self) -> Option<device::BatteryLevel> {
        None
    }
}

impl Eq for GamepadHandle {}

impl PartialEq for GamepadHandle {
    #[inline(always)]
    fn eq(&self, othr: &Self) -> bool {
        self.index == othr.index
    }
}

impl Ord for GamepadHandle {
    #[inline(always)]
    fn cmp(&self, othr: &Self) -> Ordering {
        self.index.cmp(&othr.index)
    }
}
impl PartialOrd for GamepadHandle {
    #[inline(always)]
    fn partial_cmp(&self, othr: &Self) -> Option<Ordering> {
        self.index.partial_cmp(&othr.index)
    }
}

impl Hash for GamepadHandle {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state)
    }
}

impl fmt::Debug for GamepadHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        if self.is_dummy() {
            write!(f, "GamepadHandle (Dummy)")
        } else {
            let gamepad = self.gamepad();
            write!(f, "GamepadHandle ({}#{})", gamepad.id(), gamepad.index())
        }
    }
}
