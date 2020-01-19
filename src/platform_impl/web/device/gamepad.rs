use crate::platform_impl::platform::backend;
use std::fmt;
use crate::event::device::{BatteryLevel, RumbleError};

pub enum GamepadShared {
    Raw(backend::GamepadShared),
    Dummy,
}

impl GamepadShared {
    pub fn id(&self) -> i32 {
        match self {
            GamepadShared::Raw(g) => g.index() as i32,
            GamepadShared::Dummy => -1,
        }
    }

    pub fn info(&self) -> String {
        match self {
            GamepadShared::Raw(g) => g.id(),
            GamepadShared::Dummy => String::new(),
        }
    }

    pub fn connected(&self) -> bool {
        match self {
            GamepadShared::Raw(g) => g.connected(),
            GamepadShared::Dummy => false,
        }
    }

    pub fn is_dummy(&self) -> bool {
        match self {
            GamepadShared::Dummy => true,
            _ => false,
        }
    }

    pub fn rumble(&self, _left_speed: f64, _right_speed: f64) -> Result<(), RumbleError> {
        match self {
            GamepadShared::Dummy => Ok(()),
            GamepadShared::Raw(g) => {
                g.vibrate(0.5, 2.0);
                Ok(())
            },
        }
    }

    pub fn port(&self) -> Option<u8> {
        None
    }

    pub fn battery_level(&self) -> Option<BatteryLevel> {
        None
    }
}

impl Clone for GamepadShared {
    fn clone(&self) -> Self {
        match self {
            GamepadShared::Raw(g) => GamepadShared::Raw(g.clone()),
            GamepadShared::Dummy => GamepadShared::Dummy,
        }
    }
}

impl Default for GamepadShared {
    fn default() -> Self {
        GamepadShared::Dummy
    }
}

impl fmt::Debug for GamepadShared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        if self.is_dummy() {
            write!(f, "Gamepad (Dummy)")
        } else {
            write!(
                f,
                "Gamepad ({}#{})",
                self.info(),
                self.id()
            )
        }
    }
}