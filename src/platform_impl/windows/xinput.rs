use std::mem;

use rusty_xinput::*;
use winapi::shared::minwindef::{DWORD, WORD};
use winapi::um::xinput::*;

use event::{
    ElementState,
    device::{AxisHint, ButtonHint},
};
use super::gamepad::{AxisEvent, ButtonEvent};
use platform_impl::platform::util;

lazy_static! {
    static ref XINPUT_GUARD: Option<()> = dynamic_load_xinput().ok();
}

static BUTTONS: &[(WORD , u32, ButtonHint)] = &[
    (XINPUT_GAMEPAD_DPAD_UP, 12, ButtonHint::DPadUp),
    (XINPUT_GAMEPAD_DPAD_DOWN, 13, ButtonHint::DPadDown),
    (XINPUT_GAMEPAD_DPAD_LEFT, 14, ButtonHint::DPadLeft),
    (XINPUT_GAMEPAD_DPAD_RIGHT, 15, ButtonHint::DPadRight),
    (XINPUT_GAMEPAD_START, 9, ButtonHint::Start),
    (XINPUT_GAMEPAD_BACK, 8, ButtonHint::Select),
    (XINPUT_GAMEPAD_LEFT_THUMB, 10, ButtonHint::LeftStick),
    (XINPUT_GAMEPAD_RIGHT_THUMB, 11, ButtonHint::RightStick),
    (XINPUT_GAMEPAD_LEFT_SHOULDER, 4, ButtonHint::LeftShoulder),
    (XINPUT_GAMEPAD_RIGHT_SHOULDER, 5, ButtonHint::RightShoulder),
    (XINPUT_GAMEPAD_A, 0, ButtonHint::South),
    (XINPUT_GAMEPAD_B, 1, ButtonHint::East),
    (XINPUT_GAMEPAD_X, 2, ButtonHint::West),
    (XINPUT_GAMEPAD_Y, 3, ButtonHint::North),
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
            _   => None,
        })
}

#[derive(Debug)]
enum Side {
    Left,
    Right,
}

#[derive(Debug)]
pub struct XInputGamepad {
    id: DWORD,
    prev_state: Option<XInputState>,
    state: Option<XInputState>,
}

impl XInputGamepad {
    pub fn new(id: DWORD) -> Option<Self> {
        XINPUT_GUARD.map(|_| XInputGamepad {
            id,
            prev_state: None,
            state: None,
        })
    }

    pub fn update_state(&mut self) -> Option<()> {
        let state = xinput_get_state(self.id).ok();
        if state.is_some() {
            self.prev_state = mem::replace(&mut self.state, state);
            Some(())
        } else {
            None
        }
    }

    fn check_trigger_digital(
        events: &mut Vec<ButtonEvent>,
        value: bool,
        prev_value: Option<bool>,
        side: Side,
    ) {
        const LEFT_TRIGGER_ID: u32 = /*BUTTONS.len() as _*/ 16;
        const RIGHT_TRIGGER_ID: u32 = LEFT_TRIGGER_ID + 1;
        if Some(value) != prev_value {
            let state = if value { ElementState::Pressed } else { ElementState::Released };
            let (id, hint) = match side {
                Side::Left => (LEFT_TRIGGER_ID, ButtonHint::LeftTrigger),
                Side::Right => (RIGHT_TRIGGER_ID, ButtonHint::RightTrigger),
            };
            events.push(ButtonEvent::new(id, Some(hint), state));
        }
    }

    pub fn get_changed_buttons(&self) -> Vec<ButtonEvent> {
        let (buttons, left_trigger, right_trigger) = match self.state.as_ref() {
            Some(state) => (
                state.raw.Gamepad.wButtons,
                state.left_trigger_bool(),
                state.right_trigger_bool(),
            ),
            None => return Vec::with_capacity(0),
        };
        let (prev_buttons, prev_left, prev_right) = self.prev_state
            .as_ref()
            .map(|state| (
                state.raw.Gamepad.wButtons,
                Some(state.left_trigger_bool()),
                Some(state.right_trigger_bool()),
            ))
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
        let mut events = Vec::with_capacity(BUTTONS.len() + 2);
        for &(flag, id, hint) in BUTTONS {
            if util::has_flag(pressed, flag) {
                events.push(ButtonEvent::new(id, Some(hint), ElementState::Pressed));
            } else if util::has_flag(released, flag) {
                events.push(ButtonEvent::new(id, Some(hint), ElementState::Released));
            }
        }
        Self::check_trigger_digital(&mut events, left_trigger, prev_left, Side::Left);
        Self::check_trigger_digital(&mut events, right_trigger, prev_right, Side::Right);
        events
    }

    fn check_trigger(
        events: &mut Vec<AxisEvent>,
        value: u8,
        prev_value: Option<u8>,
        side: Side,
    ) {
        const LEFT_TRIGGER_ID: u32 = 4;
        const RIGHT_TRIGGER_ID: u32 = LEFT_TRIGGER_ID + 1;
        if Some(value) != prev_value {
            let (id, hint) = match side {
                Side::Left => (LEFT_TRIGGER_ID, AxisHint::LeftTrigger),
                Side::Right => (RIGHT_TRIGGER_ID, AxisHint::RightTrigger),
            };
            events.push(AxisEvent::new(id, Some(hint), value as f64 / u8::max_value() as f64));
        }
    }

    fn check_axis(
        events: &mut Vec<AxisEvent>,
        value: i16,
        prev_value: Option<i16>,
        id: u32,
        hint: AxisHint,
    ) {
        if Some(value) != prev_value {
            let value = match value.signum() {
                 0 => 0.0,
                 1 => value as f64 / i16::max_value() as f64,
                -1 => value as f64 / (i16::min_value() as f64).abs(),
                 _ => unreachable!()
            };
            events.push(AxisEvent::new(id, Some(hint), f64::from(value)));
        }
    }

    fn check_stick(
        events: &mut Vec<AxisEvent>,
        value: (i16, i16),
        prev_value: Option<(i16, i16)>,
        stick: Side,
    ) {
        let (id, hint) = match stick {
            Side::Left => ((0, 1), (AxisHint::LeftStickX, AxisHint::LeftStickY)),
            Side::Right => ((2, 3), (AxisHint::RightStickX, AxisHint::RightStickY)),
        };
        let prev_x = prev_value.map(|prev| prev.0);
        let prev_y = prev_value.map(|prev| prev.1);
        Self::check_axis(events, value.0, prev_x, id.0, hint.0);
        Self::check_axis(events, value.1, prev_y, id.1, hint.1);
    }

    pub fn get_changed_axes(&self) -> Vec<AxisEvent> {
        let state = match self.state {
            Some(ref state) => state,
            None => return Vec::new(),
        };
        let left_stick = state.left_stick_raw();
        let right_stick = state.right_stick_raw();
        let left_trigger = state.left_trigger();
        let right_trigger = state.right_trigger();

        let mut events = Vec::with_capacity(6);

        let prev_state = self.prev_state.as_ref();
        let prev_left_stick = prev_state.map(|state| state.left_stick_raw());
        let prev_right_stick = prev_state.map(|state| state.right_stick_raw());
        let prev_left_trigger = prev_state.map(|state| state.left_trigger());
        let prev_right_trigger = prev_state.map(|state| state.right_trigger());

        Self::check_stick(&mut events, left_stick, prev_left_stick, Side::Left);
        Self::check_stick(&mut events, right_stick, prev_right_stick, Side::Right);
        Self::check_trigger(&mut events, left_trigger, prev_left_trigger, Side::Left);
        Self::check_trigger(&mut events, right_trigger, prev_right_trigger, Side::Right);
        events
    }

    pub fn rumble(&mut self, left_speed: u16, right_speed: u16) {
        // TODO: We should probably return the status
        let _ = xinput_set_state(self.id, left_speed, right_speed);
    }
}
