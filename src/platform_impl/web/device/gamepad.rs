use crate::event::device::{BatteryLevel, RumbleError};
use crate::platform_impl::platform::backend;
use std::fmt;

pub enum SharedGamepad {
    Raw(backend::SharedGamepad),
    Dummy,
}

impl SharedGamepad {
    pub fn id(&self) -> i32 {
        match self {
            SharedGamepad::Raw(g) => g.index() as i32,
            SharedGamepad::Dummy => -1,
        }
    }

    pub fn info(&self) -> String {
        match self {
            SharedGamepad::Raw(g) => g.id(),
            SharedGamepad::Dummy => String::new(),
        }
    }

    pub fn connected(&self) -> bool {
        match self {
            SharedGamepad::Raw(g) => g.connected(),
            SharedGamepad::Dummy => false,
        }
    }

    pub fn is_dummy(&self) -> bool {
        match self {
            SharedGamepad::Dummy => true,
            _ => false,
        }
    }

    pub fn rumble(&self, _left_speed: f64, _right_speed: f64) -> Result<(), RumbleError> {
        match self {
            SharedGamepad::Dummy => Ok(()),
            SharedGamepad::Raw(g) => {
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

impl Clone for SharedGamepad {
    fn clone(&self) -> Self {
        match self {
            SharedGamepad::Raw(g) => SharedGamepad::Raw(g.clone()),
            SharedGamepad::Dummy => SharedGamepad::Dummy,
        }
    }
}

impl Default for SharedGamepad {
    fn default() -> Self {
        SharedGamepad::Dummy
    }
}

impl fmt::Debug for SharedGamepad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        if self.is_dummy() {
            write!(f, "Gamepad (Dummy)")
        } else {
            write!(f, "Gamepad ({}#{})", self.info(), self.id())
        }
    }
}
