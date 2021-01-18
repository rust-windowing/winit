use std::sync::Weak;

use winapi::um::winnt::HANDLE;

use crate::{
    event::device::{BatteryLevel, GamepadEvent, RumbleError},
    platform_impl::platform::raw_input::{get_raw_input_device_name, RawGamepad},
    platform_impl::platform::xinput::{self, XInputGamepad, XInputGamepadShared},
};

#[derive(Debug)]
pub enum GamepadType {
    Raw(RawGamepad),
    XInput(XInputGamepad),
}

#[derive(Clone)]
pub enum GamepadShared {
    Raw(()),
    XInput(Weak<XInputGamepadShared>),
    Dummy,
}

#[derive(Debug)]
pub struct Gamepad {
    handle: HANDLE,
    backend: GamepadType,
}

impl Gamepad {
    pub fn new(handle: HANDLE) -> Option<Self> {
        // TODO: Verify that this is an HID device
        let name = get_raw_input_device_name(handle)?;
        xinput::id_from_name(&name)
            .and_then(XInputGamepad::new)
            .map(GamepadType::XInput)
            .or_else(|| RawGamepad::new(handle).map(GamepadType::Raw))
            .map(|backend| Gamepad { handle, backend })
    }

    pub unsafe fn update_state(&mut self, raw_input_report: &mut [u8]) -> Option<()> {
        match self.backend {
            GamepadType::Raw(ref mut gamepad) => gamepad.update_state(raw_input_report),
            GamepadType::XInput(ref mut gamepad) => gamepad.update_state(),
        }
    }

    pub fn get_gamepad_events(&self) -> Vec<GamepadEvent> {
        match self.backend {
            GamepadType::Raw(ref gamepad) => gamepad.get_gamepad_events(),
            GamepadType::XInput(ref gamepad) => gamepad.get_gamepad_events(),
        }
    }

    pub fn shared_data(&self) -> GamepadShared {
        match self.backend {
            GamepadType::Raw(_) => GamepadShared::Raw(()),
            GamepadType::XInput(ref gamepad) => GamepadShared::XInput(gamepad.shared_data()),
        }
    }
}

impl GamepadShared {
    pub fn rumble(&self, left_speed: f64, right_speed: f64) -> Result<(), RumbleError> {
        match self {
            GamepadShared::Raw(_) | GamepadShared::Dummy => Ok(()),
            GamepadShared::XInput(ref data) => data
                .upgrade()
                .map(|r| r.rumble(left_speed, right_speed))
                .unwrap_or(Err(RumbleError::DeviceNotConnected)),
        }
    }

    pub fn port(&self) -> Option<u8> {
        match self {
            GamepadShared::Raw(_) | GamepadShared::Dummy => None,
            GamepadShared::XInput(ref data) => data.upgrade().map(|r| r.port()),
        }
    }

    pub fn battery_level(&self) -> Option<BatteryLevel> {
        match self {
            GamepadShared::Raw(_) | GamepadShared::Dummy => None,
            GamepadShared::XInput(ref data) => data.upgrade().and_then(|r| r.battery_level()),
        }
    }
}
