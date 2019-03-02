use winapi::um::winnt::HANDLE;

use event::{
    ElementState,
    device::{AxisHint, ButtonHint},
};
use platform_impl::platform::raw_input::{get_raw_input_device_name, RawGamepad};
use platform_impl::platform::xinput::{self, XInputGamepad};

#[derive(Debug)]
pub enum GamepadType {
    Raw(RawGamepad),
    XInput(XInputGamepad),
}

#[derive(Debug)]
pub struct Gamepad {
    handle: HANDLE,
    backend: GamepadType,
}

#[derive(Debug, Clone, Copy)]
pub struct AxisEvent {
    pub axis: u32,
    pub hint: Option<AxisHint>,
    pub value: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct ButtonEvent {
    pub button_id: u32,
    pub hint: Option<ButtonHint>,
    pub state: ElementState,
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

    pub fn get_changed_buttons(&self) -> Vec<ButtonEvent> {
        match self.backend {
            GamepadType::Raw(ref gamepad) => gamepad.get_changed_buttons(),
            GamepadType::XInput(ref gamepad) => gamepad.get_changed_buttons(),
        }
    }

    pub fn get_changed_axes(&self) -> Vec<AxisEvent> {
        match self.backend {
            GamepadType::Raw(ref gamepad) => gamepad.get_changed_axes(),
            GamepadType::XInput(ref gamepad) => gamepad.get_changed_axes(),
        }
    }

    pub fn rumble(&mut self, left_speed: u16, right_speed: u16) {
        match self.backend {
            GamepadType::Raw(ref mut gamepad) => gamepad.rumble(left_speed, right_speed),
            GamepadType::XInput(ref mut gamepad) => gamepad.rumble(left_speed, right_speed),
        }
    }
}

impl AxisEvent {
    pub fn new(axis: u32, hint: Option<AxisHint>, value: f64) -> AxisEvent {
        AxisEvent{ axis, hint, value }
    }
}

impl ButtonEvent {
    pub fn new(button_id: u32, hint: Option<ButtonHint>, state: ElementState) -> ButtonEvent {
        ButtonEvent{ button_id, hint, state }
    }
}
