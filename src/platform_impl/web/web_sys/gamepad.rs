use crate::event::ElementState;
use crate::platform_impl::platform::device::gamepad::{native_ev_codes, EventCode};
use std::{cmp::PartialEq, rc::Rc};
use web_sys::{GamepadButton, GamepadMappingType};

pub struct Gamepad {
    pub(crate) index: u32,
    pub(crate) raw: web_sys::Gamepad,
    pub(crate) mapping: Mapping,
}

impl Gamepad {
    pub fn new(raw: web_sys::Gamepad) -> Self {
        let mapping = Mapping::new(&raw);

        Self {
            index: raw.index(),
            raw,
            mapping,
        }
    }

    pub fn raw(&self) -> web_sys::Gamepad {
        self.raw.clone()
    }

    // An integer that is auto-incremented to be unique for each device
    // currently connected to the system.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/index
    pub fn index(&self) -> u32 {
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
    pub fn vibrate(&self, _value: f64, _duration: f64) {
        //     for actuator in self.raw.haptic_actuators().values() {
        //         actuator
        //         .ok()
        //         .and_then(|a| match a.type_ {
        //             web_sys::GamepadHapticActuatorType::Vibration => {
        //                 a.pulse(value, duration);
        //                 Some(())
        //             },
        //             _ => None,
        //         });
        //     }
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

#[derive(Clone)]
pub enum Mapping {
    Standard { buttons: [bool; 17], axes: [f64; 4] },
    NoMapping { buttons: Vec<bool>, axes: Vec<f64> },
}

impl Mapping {
    pub fn new(raw: &web_sys::Gamepad) -> Mapping {
        match raw.mapping() {
            GamepadMappingType::Standard => {
                let mut buttons = [false; 17];
                let mut axes = [0.0; 4];

                let gbuttons = raw.buttons();
                for index in 0..buttons.len() {
                    let button: GamepadButton = gbuttons.get(index as u32).into();
                    buttons[index] = button.pressed();
                }

                let gaxes = raw.axes();
                for index in 0..axes.len() {
                    let axe: f64 = gaxes.get(index as u32).as_f64().unwrap_or(0.0);
                    axes[index] = axe;
                }

                Mapping::Standard { buttons, axes }
            }
            _ => {
                let mut buttons: Vec<bool> = Vec::new();
                let mut axes: Vec<f64> = Vec::new();

                let gbuttons = raw.buttons();
                for index in 0..gbuttons.length() {
                    let button: GamepadButton = gbuttons.get(index as u32).into();
                    buttons.push(button.pressed());
                }

                let gaxes = raw.axes();
                for index in 0..gaxes.length() {
                    let axe: f64 = gaxes.get(index as u32).as_f64().unwrap_or(0.0);
                    axes.push(axe);
                }

                Mapping::NoMapping { buttons, axes }
            }
        }
    }

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
