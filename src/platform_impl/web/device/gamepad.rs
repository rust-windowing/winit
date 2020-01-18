use super::super::event_loop::EventLoop;
use crate::event::device;
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    hash::{Hash, Hasher},
    fmt,
};

#[derive(Clone)]
pub(crate) struct GamepadHandle;

unsafe impl Send for GamepadHandle {}
unsafe impl Sync for GamepadHandle {}

impl GamepadHandle {
    pub unsafe fn dummy() -> Self {
        Self {}
    }

    pub fn persistent_identifier(&self) -> Option<String> {
        // raw_input::get_raw_input_device_name(self.0)
        None
    }

    pub fn is_connected(&self) -> bool {
        // raw_input::get_raw_input_device_info(self.0).is_some()
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

impl fmt::Debug for GamepadHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_tuple("GamepadHandle").finish()//.field(&self.handle).finish()
    }
}

impl Eq for GamepadHandle {}
impl PartialEq for GamepadHandle {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        false
    }
}

impl Ord for GamepadHandle {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        Ordering::Equal
    }
}
impl PartialOrd for GamepadHandle {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        None
    }
}

impl Hash for GamepadHandle {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        // self.handle.hash(state);
    }
}

#[derive(Debug)]
pub(crate) struct Gamepad;