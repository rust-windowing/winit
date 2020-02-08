mod manager;
mod mapping;
mod utils;

pub mod constants;
pub use manager::Manager;
pub use mapping::Mapping;

use crate::event::device::{BatteryLevel, RumbleError};
use crate::platform_impl::platform::backend;
use std::fmt;

pub enum Shared {
    Raw(backend::gamepad::Gamepad),
    Dummy,
}

impl Shared {
    // An integer that is auto-incremented to be unique for each device
    // currently connected to the system.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/index
    pub fn id(&self) -> i32 {
        match self {
            Shared::Raw(g) => g.index() as i32,
            Shared::Dummy => -1,
        }
    }

    // A string containing some information about the controller.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/id
    pub fn info(&self) -> String {
        match self {
            Shared::Raw(g) => g.id(),
            Shared::Dummy => String::new(),
        }
    }

    // A boolean indicating whether the gamepad is still connected to the system.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/connected
    pub fn connected(&self) -> bool {
        match self {
            Shared::Raw(g) => g.connected(),
            Shared::Dummy => false,
        }
    }

    // [EXPERIMENTAL] An array containing GamepadHapticActuator objects,
    // each of which represents haptic feedback hardware available on the controller.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/hapticActuators
    pub fn rumble(&self, left_speed: f64, _right_speed: f64) -> Result<(), RumbleError> {
        match self {
            Shared::Dummy => Ok(()),
            Shared::Raw(g) => {
                g.vibrate(left_speed, 1000f64);
                Ok(())
            }
        }
    }

    pub fn is_dummy(&self) -> bool {
        match self {
            Shared::Dummy => true,
            _ => false,
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
