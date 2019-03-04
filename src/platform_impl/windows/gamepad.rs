use std::sync::Weak;

use winapi::um::winnt::HANDLE;

use event::device::GamepadEvent;
use platform_impl::platform::raw_input::{get_raw_input_device_name, RawGamepad};
use platform_impl::platform::xinput::{self, XInputGamepad, XInputGamepadRumbler};

#[derive(Debug)]
pub enum GamepadType {
    Raw(RawGamepad),
    XInput(XInputGamepad),
}

#[derive(Clone)]
pub enum GamepadRumbler {
    Raw(()),
    XInput(Weak<XInputGamepadRumbler>),
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
            .or_else(||
                RawGamepad::new(handle).map(GamepadType::Raw)
            )
            .map(|backend| Gamepad {
                handle,
                backend,
            })
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

    pub fn rumbler(&self) -> GamepadRumbler {
        match self.backend {
            GamepadType::Raw(_) => GamepadRumbler::Raw(()),
            GamepadType::XInput(ref gamepad) => GamepadRumbler::XInput(gamepad.rumbler()),
        }
    }
}

impl GamepadRumbler {
    pub fn rumble(&self, left_speed: f64, right_speed: f64) {
        match self {
            GamepadRumbler::Raw(_) => (),
            GamepadRumbler::XInput(ref rumbler) => {rumbler.upgrade().map(|r| r.rumble(left_speed, right_speed));},
            GamepadRumbler::Dummy => (),
        }
    }
}
