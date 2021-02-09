use super::utils;
use crate::platform_impl::platform::device;
use std::cmp::PartialEq;
use stdweb::js;

#[derive(Debug)]
pub struct Gamepad {
    pub(crate) index: i32,
    pub(crate) raw: stdweb::web::Gamepad,
    pub(crate) mapping: device::gamepad::Mapping,
}

impl Gamepad {
    pub fn new(raw: stdweb::web::Gamepad) -> Self {
        let mapping = utils::create_mapping(&raw);

        Self {
            index: raw.index(),
            raw,
            mapping,
        }
    }

    // An integer that is auto-incremented to be unique for each device
    // currently connected to the system.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/index
    pub fn index(&self) -> i32 {
        self.raw.index()
    }

    // A string containing some information about the controller.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/id
    pub fn id(&self) -> String {
        self.raw.id()
    }

    // A boolean indicating whether the gamepad is still connected to the system.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/connected
    pub fn connected(&self) -> bool {
        self.raw.connected()
    }

    // EXPERIMENTAL
    #[allow(dead_code)]
    pub fn vibrate(&self, value: f64, duration: f64) {
        let index = self.index;
        js! {
            const gamepads = navigator.getGamepads();
            let gamepad = null;
            for (let i = 0; i < gamepads.length; i++) {
                if (gamepads[i] && gamepads[i].index == @{index}) {
                    gamepad = gamepads[i];
                    break
                }
            }
            if (!gamepad || !gamepad.hapticActuators) return;
            for (let i = 0; i < gamepad.hapticActuators.length; i++) {
                const actuator = gamepad.hapticActuators[i];
                if (actuator && actuator.type === "vibration") {
                    actuator.pulse(@{value}, @{duration});
                }
            }
        }
    }
}

impl Clone for Gamepad {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            raw: self.raw.clone(),
            mapping: self.mapping.clone(),
        }
    }
}

impl PartialEq for Gamepad {
    #[inline(always)]
    fn eq(&self, othr: &Self) -> bool {
        self.raw.index() == othr.raw.index()
    }
}
