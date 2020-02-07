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


