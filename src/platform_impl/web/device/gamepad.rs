use crate::event::device::{BatteryLevel, GamepadAxis, GamepadButton, RumbleError};
use crate::platform_impl::platform::backend;
use std::fmt;

pub enum Shared {
    Raw(backend::gamepad::Gamepad),
    Dummy,
}

impl Shared {
    pub fn id(&self) -> i32 {
        match self {
            Shared::Raw(g) => g.index() as i32,
            Shared::Dummy => -1,
        }
    }

    pub fn info(&self) -> String {
        match self {
            Shared::Raw(g) => g.id(),
            Shared::Dummy => String::new(),
        }
    }

    pub fn connected(&self) -> bool {
        match self {
            Shared::Raw(g) => g.connected(),
            Shared::Dummy => false,
        }
    }

    pub fn is_dummy(&self) -> bool {
        match self {
            Shared::Dummy => true,
            _ => false,
        }
    }

    pub fn rumble(&self, _left_speed: f64, _right_speed: f64) -> Result<(), RumbleError> {
        match self {
            Shared::Dummy => Ok(()),
            Shared::Raw(g) => {
                g.vibrate(0.5, 2.0);
                Ok(())
            }
        }
    }

    pub fn port(&self) -> Option<u8> {
        None
    }

    pub fn battery_level(&self) -> Option<BatteryLevel> {
        None
    }
}

impl Clone for Shared {
    fn clone(&self) -> Self {
        match self {
            Shared::Raw(g) => Shared::Raw(g.clone()),
            Shared::Dummy => Shared::Dummy,
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Shared::Dummy
    }
}

impl fmt::Debug for Shared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        if self.is_dummy() {
            write!(f, "Gamepad (Dummy)")
        } else {
            write!(f, "Gamepad ({}#{})", self.id(), self.info())
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EventCode(pub(crate) u8);

impl From<EventCode> for Option<GamepadButton> {
    fn from(code: EventCode) -> Self {
        match code {
            native_ev_codes::BTN_SOUTH => Some(GamepadButton::South),
            native_ev_codes::BTN_EAST => Some(GamepadButton::East),
            // native_ev_codes::BTN_C => Some(GamepadButton::C),
            native_ev_codes::BTN_NORTH => Some(GamepadButton::North),
            native_ev_codes::BTN_WEST => Some(GamepadButton::West),
            // native_ev_codes::BTN_Z => Some(GamepadButton::Z),
            native_ev_codes::BTN_LT => Some(GamepadButton::LeftTrigger),
            native_ev_codes::BTN_RT => Some(GamepadButton::RightTrigger),
            native_ev_codes::BTN_LT2 => Some(GamepadButton::LeftShoulder),
            native_ev_codes::BTN_RT2 => Some(GamepadButton::RightShoulder),
            native_ev_codes::BTN_SELECT => Some(GamepadButton::Select),
            native_ev_codes::BTN_START => Some(GamepadButton::Start),
            // native_ev_codes::BTN_MODE => Some(GamepadButton::MODE),
            native_ev_codes::BTN_LTHUMB => Some(GamepadButton::LeftStick),
            native_ev_codes::BTN_RTHUMB => Some(GamepadButton::RightStick),
            native_ev_codes::BTN_DPAD_UP => Some(GamepadButton::DPadUp),
            native_ev_codes::BTN_DPAD_DOWN => Some(GamepadButton::DPadDown),
            native_ev_codes::BTN_DPAD_LEFT => Some(GamepadButton::DPadLeft),
            native_ev_codes::BTN_DPAD_RIGHT => Some(GamepadButton::DPadRight),
            _ => None,
        }
    }
}

impl From<EventCode> for Option<GamepadAxis> {
    fn from(code: EventCode) -> Self {
        match code {
            native_ev_codes::AXIS_LSTICKX => Some(GamepadAxis::LeftStickX),
            native_ev_codes::AXIS_LSTICKY => Some(GamepadAxis::LeftStickY),
            // native_ev_codes::AXIS_LEFTZ => Some(GamepadAxis::LeftZ),
            native_ev_codes::AXIS_RSTICKX => Some(GamepadAxis::RightStickX),
            native_ev_codes::AXIS_RSTICKY => Some(GamepadAxis::RightStickY),
            // native_ev_codes::AXIS_RIGHTZ => Some(GamepadAxis::RightZ),
            // native_ev_codes::AXIS_DPADX => Some(GamepadAxis::DPadX),
            // native_ev_codes::AXIS_DPADY => Some(GamepadAxis::DPadY),
            native_ev_codes::AXIS_RT => Some(GamepadAxis::RightTrigger),
            native_ev_codes::AXIS_LT => Some(GamepadAxis::LeftTrigger),
            // native_ev_codes::AXIS_RT2 => Some(GamepadAxis::LeftShoulder),
            // native_ev_codes::AXIS_LT2 => Some(GamepadAxis::RightShoulder),
            _ => None,
        }
    }
}

impl fmt::Display for EventCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub mod native_ev_codes {
    use super::EventCode;

    pub const AXIS_LSTICKX: EventCode = EventCode(0);
    pub const AXIS_LSTICKY: EventCode = EventCode(1);
    pub const AXIS_LEFTZ: EventCode = EventCode(2);
    pub const AXIS_RSTICKX: EventCode = EventCode(3);
    pub const AXIS_RSTICKY: EventCode = EventCode(4);
    pub const AXIS_RIGHTZ: EventCode = EventCode(5);
    pub const AXIS_DPADX: EventCode = EventCode(6);
    pub const AXIS_DPADY: EventCode = EventCode(7);
    pub const AXIS_RT: EventCode = EventCode(8);
    pub const AXIS_LT: EventCode = EventCode(9);
    pub const AXIS_RT2: EventCode = EventCode(10);
    pub const AXIS_LT2: EventCode = EventCode(11);

    pub const BTN_SOUTH: EventCode = EventCode(12);
    pub const BTN_EAST: EventCode = EventCode(13);
    pub const BTN_C: EventCode = EventCode(14);
    pub const BTN_NORTH: EventCode = EventCode(15);
    pub const BTN_WEST: EventCode = EventCode(16);
    pub const BTN_Z: EventCode = EventCode(17);
    pub const BTN_LT: EventCode = EventCode(18);
    pub const BTN_RT: EventCode = EventCode(19);
    pub const BTN_LT2: EventCode = EventCode(20);
    pub const BTN_RT2: EventCode = EventCode(21);
    pub const BTN_SELECT: EventCode = EventCode(22);
    pub const BTN_START: EventCode = EventCode(23);
    pub const BTN_MODE: EventCode = EventCode(24);
    pub const BTN_LTHUMB: EventCode = EventCode(25);
    pub const BTN_RTHUMB: EventCode = EventCode(26);

    pub const BTN_DPAD_UP: EventCode = EventCode(27);
    pub const BTN_DPAD_DOWN: EventCode = EventCode(28);
    pub const BTN_DPAD_LEFT: EventCode = EventCode(29);
    pub const BTN_DPAD_RIGHT: EventCode = EventCode(30);

    pub(crate) static BUTTONS: [EventCode; 17] = [
        BTN_SOUTH,
        BTN_EAST,
        BTN_NORTH,
        BTN_WEST,
        BTN_LT,
        BTN_RT,
        BTN_LT2,
        BTN_RT2,
        BTN_SELECT,
        BTN_START,
        BTN_LTHUMB,
        BTN_RTHUMB,
        BTN_DPAD_UP,
        BTN_DPAD_DOWN,
        BTN_DPAD_LEFT,
        BTN_DPAD_RIGHT,
        BTN_MODE,
    ];

    pub(crate) static AXES: [EventCode; 4] =
        [AXIS_LSTICKX, AXIS_LSTICKY, AXIS_RSTICKX, AXIS_RSTICKY];

    pub(crate) fn button_code(index: usize) -> EventCode {
        BUTTONS
            .get(index)
            .map(|ev| ev.clone())
            .unwrap_or(EventCode(index as u8 + 31))
    }

    pub(crate) fn axis_code(index: usize) -> EventCode {
        AXES
            .get(index)
            .map(|ev| ev.clone())
            .unwrap_or(EventCode((index + BUTTONS.len()) as u8 + 31))
    }
}
