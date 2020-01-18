use super::super::event_loop::EventLoop;
use crate::event::device;
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    hash::{Hash, Hasher},
    fmt,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GamepadHandle(pub i32);

unsafe impl Send for GamepadHandle {}
unsafe impl Sync for GamepadHandle {}

impl GamepadHandle {
    pub unsafe fn dummy() -> Self {
        Self(0)
    }

    pub fn is_connected(&self) -> bool {
        false
    }

    pub fn enumerate<'a, T>(
        event_loop: &'a EventLoop<T>,
    ) -> impl 'a + Iterator<Item = device::GamepadHandle> {
        event_loop.gamepads()
    }

    pub fn rumble(&self, left_speed: f64, right_speed: f64) -> Result<(), device::RumbleError> {
        Ok(())
    }

    pub fn port(&self) -> Option<u8> {
        None
    }

    pub fn battery_level(&self) -> Option<device::BatteryLevel> {
        None
    }
}

impl From<GamepadHandle> for device::GamepadHandle {
    fn from(platform_id: GamepadHandle) -> Self {
        Self(platform_id)
    }
}

#[derive(Debug)]
pub(crate) struct Gamepad;