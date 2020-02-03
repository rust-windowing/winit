use std::rc::Rc;

pub struct Shared(pub(crate) Rc<web_sys::Gamepad>);

impl Shared {
    // An integer that is auto-incremented to be unique for each device
    // currently connected to the system.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/index
    pub fn index(&self) -> u32 {
        self.0.index()
    }

    // A string containing some information about the controller.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/id
    pub fn id(&self) -> String {
        self.0.id()
    }

    // A boolean indicating whether the gamepad is still connected to the system.
    // https://developer.mozilla.org/en-US/docs/Web/API/Gamepad/connected
    pub fn connected(&self) -> bool {
        self.0.connected()
    }

    // EXPERIMENTAL
    #[allow(dead_code)]
    pub fn vibrate(&self, _value: f64, _duration: f64) {
        //     for actuator in self.0.haptic_actuators().values() {
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

impl Clone for Shared {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}
