mod gamepad;

use super::event_loop::EventLoop;
use crate::event::device;
pub use gamepad::GamepadShared;
use std::{
    cmp::{Eq, Ordering, PartialEq, PartialOrd},
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

#[derive(Clone, Debug)]
pub(crate) struct GamepadHandle {
    pub(crate) id: i32,
    pub(crate) gamepad: GamepadShared,
}

unsafe impl Send for GamepadHandle {}
unsafe impl Sync for GamepadHandle {}

impl GamepadHandle {
    pub unsafe fn dummy() -> Self {
        Self {
            id: -1,
            gamepad: GamepadShared::default(),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.gamepad.connected()
    }

    pub fn enumerate<'a, T>(
        event_loop: &'a EventLoop<T>,
    ) -> impl 'a + Iterator<Item = device::GamepadHandle> {
        event_loop.gamepads()
    }

    pub fn rumble(&self, left_speed: f64, right_speed: f64) -> Result<(), device::RumbleError> {
        self.gamepad.rumble(left_speed, right_speed)
    }

    pub fn port(&self) -> Option<u8> {
        self.gamepad.port()
    }

    pub fn battery_level(&self) -> Option<device::BatteryLevel> {
        self.gamepad.battery_level()
    }
}

impl Eq for GamepadHandle {}

impl PartialEq for GamepadHandle {
    #[inline(always)]
    fn eq(&self, othr: &Self) -> bool {
        self.id == othr.id
    }
}

impl Ord for GamepadHandle {
    #[inline(always)]
    fn cmp(&self, othr: &Self) -> Ordering {
        self.id.cmp(&othr.id)
    }
}
impl PartialOrd for GamepadHandle {
    #[inline(always)]
    fn partial_cmp(&self, othr: &Self) -> Option<Ordering> {
        self.id.partial_cmp(&othr.id)
    }
}

impl Hash for GamepadHandle {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}
