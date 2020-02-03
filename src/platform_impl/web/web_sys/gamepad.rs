use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct SharedGamepadManager(Rc<GamepadManager>);

pub struct GamepadManager {
    gamepads: RefCell<HashMap<u32, SharedGamepad>>,
}

pub struct SharedGamepad(Rc<web_sys::Gamepad>);

impl SharedGamepadManager {
    pub fn create() -> SharedGamepadManager {
        SharedGamepadManager(Rc::new(GamepadManager {
            gamepads: RefCell::new(HashMap::new()),
        }))
    }

    pub fn manager(&self) -> Rc<GamepadManager> {
        self.0.clone()
    }
}

impl Clone for SharedGamepadManager {
    fn clone(&self) -> Self {
        SharedGamepadManager(self.0.clone())
    }
}

impl GamepadManager {
    pub fn register(&self, gamepad: web_sys::Gamepad) -> SharedGamepad {
        let index = gamepad.index();
        let mut gamepads = self.gamepads.borrow_mut();
        if !gamepads.contains_key(&index) {
            gamepads.insert(index, SharedGamepad(Rc::new(gamepad)));
        }
        gamepads
            .get(&index)
            .map(|g| g.clone())
            .expect("[register] Gamepad expected")
    }

    pub fn get(&self, index: &u32) -> Option<SharedGamepad> {
        self.gamepads.borrow().get(index).map(|g| g.clone())
    }
}

impl SharedGamepad {
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

impl Clone for SharedGamepad {
    fn clone(&self) -> Self {
        SharedGamepad(self.0.clone())
    }
}
