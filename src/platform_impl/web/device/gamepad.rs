use crate::event::device::{BatteryLevel, GamepadAxis, GamepadButton, RumbleError};
use crate::platform_impl::platform::backend;
use std::fmt;
use super::constants;

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

    // [EXPERIMENTAL] Not implemented yet
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
            constants::BTN_SOUTH => Some(GamepadButton::South),
            constants::BTN_EAST => Some(GamepadButton::East),
            // constants::BTN_C => Some(GamepadButton::C),
            constants::BTN_NORTH => Some(GamepadButton::North),
            constants::BTN_WEST => Some(GamepadButton::West),
            // constants::BTN_Z => Some(GamepadButton::Z),
            constants::BTN_LT => Some(GamepadButton::LeftTrigger),
            constants::BTN_RT => Some(GamepadButton::RightTrigger),
            constants::BTN_LT2 => Some(GamepadButton::LeftShoulder),
            constants::BTN_RT2 => Some(GamepadButton::RightShoulder),
            constants::BTN_SELECT => Some(GamepadButton::Select),
            constants::BTN_START => Some(GamepadButton::Start),
            // constants::BTN_MODE => Some(GamepadButton::MODE),
            constants::BTN_LTHUMB => Some(GamepadButton::LeftStick),
            constants::BTN_RTHUMB => Some(GamepadButton::RightStick),
            constants::BTN_DPAD_UP => Some(GamepadButton::DPadUp),
            constants::BTN_DPAD_DOWN => Some(GamepadButton::DPadDown),
            constants::BTN_DPAD_LEFT => Some(GamepadButton::DPadLeft),
            constants::BTN_DPAD_RIGHT => Some(GamepadButton::DPadRight),
            _ => None,
        }
    }
}

impl From<EventCode> for Option<GamepadAxis> {
    fn from(code: EventCode) -> Self {
        match code {
            constants::AXIS_LSTICKX => Some(GamepadAxis::LeftStickX),
            constants::AXIS_LSTICKY => Some(GamepadAxis::LeftStickY),
            // constants::AXIS_LEFTZ => Some(GamepadAxis::LeftZ),
            constants::AXIS_RSTICKX => Some(GamepadAxis::RightStickX),
            constants::AXIS_RSTICKY => Some(GamepadAxis::RightStickY),
            // constants::AXIS_RIGHTZ => Some(GamepadAxis::RightZ),
            // constants::AXIS_DPADX => Some(GamepadAxis::DPadX),
            // constants::AXIS_DPADY => Some(GamepadAxis::DPadY),
            constants::AXIS_RT => Some(GamepadAxis::RightTrigger),
            constants::AXIS_LT => Some(GamepadAxis::LeftTrigger),
            // constants::AXIS_RT2 => Some(GamepadAxis::LeftShoulder),
            // constants::AXIS_LT2 => Some(GamepadAxis::RightShoulder),
            _ => None,
        }
    }
}

impl fmt::Display for EventCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub enum Mapping {
    Standard { buttons: [bool; 17], axes: [f64; 4] },
    NoMapping { buttons: Vec<bool>, axes: Vec<f64> },
}

impl Mapping {
    pub(crate) fn buttons<'a>(&'a self) -> impl Iterator<Item = bool> + 'a {
        match self {
            Mapping::Standard { buttons, .. } => buttons.iter(),
            Mapping::NoMapping { buttons, .. } => buttons.iter(),
        }
        .cloned()
    }

    pub(crate) fn axes<'a>(&'a self) -> impl Iterator<Item = f64> + 'a {
        match self {
            Mapping::Standard { axes, .. } => axes.iter(),
            Mapping::NoMapping { axes, .. } => axes.iter(),
        }
        .cloned()
    }
}
