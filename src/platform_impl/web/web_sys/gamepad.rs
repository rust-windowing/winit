use super::utils;
use crate::platform_impl::platform::device;
use std::cmp::PartialEq;

#[derive(Debug)]
pub struct Gamepad {
    pub(crate) index: i32,
    pub(crate) raw: web_sys::Gamepad,
    pub(crate) mapping: device::gamepad::Mapping,
}

impl Gamepad {
    pub fn new(raw: web_sys::Gamepad) -> Self {
        let mapping = utils::create_mapping(&raw);

        Self {
            index: raw.index() as i32,
            raw,
            mapping,
        }
    }

    // An integer that is auto-incremented to be unique for each device
    // currently connected to the system.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/index
    pub fn index(&self) -> i32 {
        self.raw.index() as i32
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

    // An array containing GamepadHapticActuator objects,
    // each of which represents haptic feedback hardware available on the controller.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/hapticActuators
    pub fn vibrate(&self, value: f64, duration: f64) {
        for actuator in self.raw.haptic_actuators().values() {
            actuator.ok().and_then(|a| {
                let actuator: web_sys::GamepadHapticActuator = a.into();
                match actuator.type_() {
                    web_sys::GamepadHapticActuatorType::Vibration => {
                        actuator.pulse(value, duration).ok()
                    }
                    _ => None,
                }
            });
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
