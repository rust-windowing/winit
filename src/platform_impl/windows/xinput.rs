use std::sync::{Arc, Weak};
use std::{io, mem};

use rusty_xinput::*;
use winapi::shared::minwindef::{DWORD, WORD};
use winapi::um::xinput::*;

use crate::{
    event::{
        device::{BatteryLevel, GamepadAxis, GamepadButton, GamepadEvent, RumbleError, Side},
        ElementState,
    },
    platform_impl::platform::util,
};

lazy_static! {
    static ref XINPUT_HANDLE: Option<XInputHandle> = XInputHandle::load_default().ok();
}

static BUTTONS: &[(WORD, u32, GamepadButton)] = &[
    (XINPUT_GAMEPAD_DPAD_UP, 12, GamepadButton::DPadUp),
    (XINPUT_GAMEPAD_DPAD_DOWN, 13, GamepadButton::DPadDown),
    (XINPUT_GAMEPAD_DPAD_LEFT, 14, GamepadButton::DPadLeft),
    (XINPUT_GAMEPAD_DPAD_RIGHT, 15, GamepadButton::DPadRight),
    (XINPUT_GAMEPAD_START, 9, GamepadButton::Start),
    (XINPUT_GAMEPAD_BACK, 8, GamepadButton::Select),
    (XINPUT_GAMEPAD_LEFT_THUMB, 10, GamepadButton::LeftStick),
    (XINPUT_GAMEPAD_RIGHT_THUMB, 11, GamepadButton::RightStick),
    (XINPUT_GAMEPAD_LEFT_SHOULDER, 4, GamepadButton::LeftShoulder),
    (
        XINPUT_GAMEPAD_RIGHT_SHOULDER,
        5,
        GamepadButton::RightShoulder,
    ),
    (XINPUT_GAMEPAD_A, 0, GamepadButton::South),
    (XINPUT_GAMEPAD_B, 1, GamepadButton::East),
    (XINPUT_GAMEPAD_X, 2, GamepadButton::West),
    (XINPUT_GAMEPAD_Y, 3, GamepadButton::North),
];

pub fn id_from_name(name: &str) -> Option<DWORD> {
    // A device name looks like \\?\HID#VID_046D&PID_C21D&IG_00#8&6daf3b6&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}
    // The IG_00 substring indicates that this is an XInput gamepad, and that the ID is 00
    let pat = "IG_0";
    name.find(pat)
        .and_then(|i| name[i + pat.len()..].chars().next())
        .and_then(|c| match c {
            '0' => Some(0),
            '1' => Some(1),
            '2' => Some(2),
            '3' => Some(3),
            _ => None,
        })
}

#[derive(Debug)]
pub struct XInputGamepad {
    shared: Arc<XInputGamepadShared>,
    prev_state: Option<XInputState>,
    state: Option<XInputState>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct XInputGamepadShared {
    port: DWORD,
}

impl XInputGamepad {
    pub fn new(port: DWORD) -> Option<Self> {
        XINPUT_HANDLE.as_ref().map(|_| XInputGamepad {
            shared: Arc::new(XInputGamepadShared { port }),
            prev_state: None,
            state: None,
        })
    }

    pub fn update_state(&mut self) -> Option<()> {
        let state = XINPUT_HANDLE
            .as_ref()
            .and_then(|h| h.get_state(self.shared.port).ok());
        if state.is_some() {
            self.prev_state = mem::replace(&mut self.state, state);
            Some(())
        } else {
            None
        }
    }

    fn check_trigger_digital(
        events: &mut Vec<GamepadEvent>,
        value: bool,
        prev_value: Option<bool>,
        side: Side,
    ) {
        const LEFT_TRIGGER_ID: u32 = /*BUTTONS.len() as _*/ 16;
        const RIGHT_TRIGGER_ID: u32 = LEFT_TRIGGER_ID + 1;
        if Some(value) != prev_value {
            let state = if value {
                ElementState::Pressed
            } else {
                ElementState::Released
            };
            let (button_id, button) = match side {
                Side::Left => (LEFT_TRIGGER_ID, Some(GamepadButton::LeftTrigger)),
                Side::Right => (RIGHT_TRIGGER_ID, Some(GamepadButton::RightTrigger)),
            };
            events.push(GamepadEvent::Button {
                button_id,
                button,
                state,
            });
        }
    }

    pub fn get_changed_buttons(&self, events: &mut Vec<GamepadEvent>) {
        let (buttons, left_trigger, right_trigger) = match self.state.as_ref() {
            Some(state) => (
                state.raw.Gamepad.wButtons,
                state.left_trigger_bool(),
                state.right_trigger_bool(),
            ),
            None => return,
        };
        let (prev_buttons, prev_left, prev_right) = self
            .prev_state
            .as_ref()
            .map(|state| {
                (
                    state.raw.Gamepad.wButtons,
                    Some(state.left_trigger_bool()),
                    Some(state.right_trigger_bool()),
                )
            })
            .unwrap_or_else(|| (0, None, None));
        /*
        A = buttons
        B = prev_buttons
        C = changed
        P = pressed
        R = released
         A B  C  C A  P  C B  R
        (0 0) 0 (0 0) 0 (0 0) 0
        (0 1) 1 (1 1) 1 (1 0) 0
        (1 0) 1 (1 0) 0 (1 1) 1
        (1 1) 0 (0 1) 0 (0 1) 0
        */
        let changed = buttons ^ prev_buttons;
        let pressed = changed & buttons;
        let released = changed & prev_buttons;
        for &(flag, button_id, button) in BUTTONS {
            let button = Some(button);
            if util::has_flag(pressed, flag) {
                events.push(GamepadEvent::Button {
                    button_id,
                    button,
                    state: ElementState::Pressed,
                });
            } else if util::has_flag(released, flag) {
                events.push(GamepadEvent::Button {
                    button_id,
                    button,
                    state: ElementState::Released,
                });
            }
        }
        Self::check_trigger_digital(events, left_trigger, prev_left, Side::Left);
        Self::check_trigger_digital(events, right_trigger, prev_right, Side::Right);
    }

    fn check_trigger(
        events: &mut Vec<GamepadEvent>,
        value: u8,
        prev_value: Option<u8>,
        side: Side,
    ) {
        const LEFT_TRIGGER_ID: u32 = 4;
        const RIGHT_TRIGGER_ID: u32 = LEFT_TRIGGER_ID + 1;
        if Some(value) != prev_value {
            let (axis_id, axis) = match side {
                Side::Left => (LEFT_TRIGGER_ID, Some(GamepadAxis::LeftTrigger)),
                Side::Right => (RIGHT_TRIGGER_ID, Some(GamepadAxis::RightTrigger)),
            };
            events.push(GamepadEvent::Axis {
                axis_id,
                axis,
                value: value as f64 / u8::max_value() as f64,
                stick: false,
            });
        }
    }

    fn check_stick(
        events: &mut Vec<GamepadEvent>,
        value: (i16, i16),
        prev_value: Option<(i16, i16)>,
        stick: Side,
    ) {
        let (id, axis) = match stick {
            Side::Left => ((0, 1), (GamepadAxis::LeftStickX, GamepadAxis::LeftStickY)),
            Side::Right => ((2, 3), (GamepadAxis::RightStickX, GamepadAxis::RightStickY)),
        };
        let prev_x = prev_value.map(|prev| prev.0);
        let prev_y = prev_value.map(|prev| prev.1);

        let value_f64 = |value_int: i16| match value_int.signum() {
            0 => 0.0,
            1 => value_int as f64 / i16::max_value() as f64,
            -1 => value_int as f64 / (i16::min_value() as f64).abs(),
            _ => unreachable!(),
        };

        let value_f64 = (value_f64(value.0), value_f64(value.1));
        if prev_x != Some(value.0) {
            events.push(GamepadEvent::Axis {
                axis_id: id.0,
                axis: Some(axis.0),
                value: value_f64.0,
                stick: true,
            });
        }
        if prev_y != Some(value.1) {
            events.push(GamepadEvent::Axis {
                axis_id: id.1,
                axis: Some(axis.1),
                value: value_f64.1,
                stick: true,
            });
        }
        if prev_x != Some(value.0) || prev_y != Some(value.1) {
            events.push(GamepadEvent::Stick {
                x_id: id.0,
                y_id: id.1,
                x_value: value_f64.0,
                y_value: value_f64.1,
                side: stick,
            })
        }
    }

    pub fn get_changed_axes(&self, events: &mut Vec<GamepadEvent>) {
        let state = match self.state {
            Some(ref state) => state,
            None => return,
        };
        let left_stick = state.left_stick_raw();
        let right_stick = state.right_stick_raw();
        let left_trigger = state.left_trigger();
        let right_trigger = state.right_trigger();

        let prev_state = self.prev_state.as_ref();
        let prev_left_stick = prev_state.map(|state| state.left_stick_raw());
        let prev_right_stick = prev_state.map(|state| state.right_stick_raw());
        let prev_left_trigger = prev_state.map(|state| state.left_trigger());
        let prev_right_trigger = prev_state.map(|state| state.right_trigger());

        Self::check_stick(events, left_stick, prev_left_stick, Side::Left);
        Self::check_stick(events, right_stick, prev_right_stick, Side::Right);
        Self::check_trigger(events, left_trigger, prev_left_trigger, Side::Left);
        Self::check_trigger(events, right_trigger, prev_right_trigger, Side::Right);
    }

    pub fn get_gamepad_events(&self) -> Vec<GamepadEvent> {
        let mut events = Vec::new();
        self.get_changed_axes(&mut events);
        self.get_changed_buttons(&mut events);
        events
    }

    pub fn shared_data(&self) -> Weak<XInputGamepadShared> {
        Arc::downgrade(&self.shared)
    }
}

impl Drop for XInputGamepad {
    fn drop(&mut self) {
        // For some reason, if you don't attempt to retrieve the xinput gamepad state at least once
        // after the gamepad was disconnected, all future attempts to read from a given port (even
        // if a controller was plugged back into said port) will fail! I don't know why that happens,
        // but this fixes it, so 🤷.
        XINPUT_HANDLE
            .as_ref()
            .and_then(|h| h.get_state(self.shared.port).ok());
    }
}

impl XInputGamepadShared {
    pub fn rumble(&self, left_speed: f64, right_speed: f64) -> Result<(), RumbleError> {
        let left_speed = (left_speed.max(0.0).min(1.0) * u16::max_value() as f64) as u16;
        let right_speed = (right_speed.max(0.0).min(1.0) * u16::max_value() as f64) as u16;

        let result = XINPUT_HANDLE
            .as_ref()
            .unwrap()
            .set_state(self.port, left_speed, right_speed);
        result.map_err(|e| match e {
            XInputUsageError::XInputNotLoaded | XInputUsageError::InvalidControllerID => panic!(
                "unexpected xinput error {:?}; this is a bug and should be reported",
                e
            ),
            XInputUsageError::DeviceNotConnected => RumbleError::DeviceNotConnected,
            XInputUsageError::UnknownError(code) => {
                RumbleError::OsError(io::Error::from_raw_os_error(code as i32))
            }
        })
    }

    pub fn port(&self) -> u8 {
        self.port as _
    }

    pub fn battery_level(&self) -> Option<BatteryLevel> {
        use rusty_xinput::BatteryLevel as XBatteryLevel;

        let battery_info = XINPUT_HANDLE
            .as_ref()
            .unwrap()
            .get_gamepad_battery_information(self.port)
            .ok()?;
        match battery_info.battery_type {
            BatteryType::ALKALINE | BatteryType::NIMH => match battery_info.battery_level {
                XBatteryLevel::EMPTY => Some(BatteryLevel::Empty),
                XBatteryLevel::LOW => Some(BatteryLevel::Low),
                XBatteryLevel::MEDIUM => Some(BatteryLevel::Medium),
                XBatteryLevel::FULL => Some(BatteryLevel::Full),
                _ => None,
            },
            _ => None,
        }
    }
}
