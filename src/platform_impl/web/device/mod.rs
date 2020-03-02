pub mod gamepad;

use super::event_loop::EventLoop;
use crate::event::device;

use std::{
    cmp::{Eq, Ordering, PartialEq, PartialOrd},
    hash::{Hash, Hasher},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MouseId(pub i32);

unsafe impl Send for MouseId {}
unsafe impl Sync for MouseId {}

impl MouseId {
    pub unsafe fn dummy() -> Self {
        Self(0)
    }

    pub fn is_connected(&self) -> bool {
        false
    }

    pub fn enumerate<'a, T>(
        event_loop: &'a EventLoop<T>,
    ) -> impl 'a + Iterator<Item = device::MouseId> {
        event_loop.mice()
    }
}

impl From<MouseId> for device::MouseId {
    fn from(platform_id: MouseId) -> Self {
        Self(platform_id)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct KeyboardId(pub i32);

unsafe impl Send for KeyboardId {}
unsafe impl Sync for KeyboardId {}

impl KeyboardId {
    pub unsafe fn dummy() -> Self {
        Self(0)
    }

    pub fn is_connected(&self) -> bool {
        false
    }

    pub fn enumerate<'a, T>(
        event_loop: &'a EventLoop<T>,
    ) -> impl 'a + Iterator<Item = device::KeyboardId> {
        event_loop.keyboards()
    }
}

impl From<KeyboardId> for device::KeyboardId {
    fn from(platform_id: KeyboardId) -> Self {
        Self(platform_id)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct HidId(pub i32);

unsafe impl Send for HidId {}
unsafe impl Sync for HidId {}

impl HidId {
    pub unsafe fn dummy() -> Self {
        Self(0)
    }

    pub fn is_connected(&self) -> bool {
        false
    }

    pub fn enumerate<'a, T>(
        event_loop: &'a EventLoop<T>,
    ) -> impl 'a + Iterator<Item = device::HidId> {
        event_loop.hids()
    }
}

impl From<HidId> for device::HidId {
    fn from(platform_id: HidId) -> Self {
        Self(platform_id)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GamepadHandle {
    pub(crate) id: i32,
    pub(crate) gamepad: gamepad::Shared,
}

unsafe impl Send for GamepadHandle {}
unsafe impl Sync for GamepadHandle {}

impl GamepadHandle {
    pub unsafe fn dummy() -> Self {
        Self {
            id: -1,
            gamepad: gamepad::Shared::default(),
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
