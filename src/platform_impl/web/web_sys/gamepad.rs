use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct GamepadManagerShared(Rc<GamepadManager>);

pub struct GamepadManager {
    gamepads: RefCell<HashMap<u32, GamepadShared>>,
}

pub struct GamepadShared(Rc<web_sys::Gamepad>);

impl GamepadManagerShared {
    pub fn create() -> GamepadManagerShared {
        GamepadManagerShared(Rc::new(GamepadManager {
            gamepads: RefCell::new(HashMap::new()),
        }))
    }

    pub fn register(&self, gamepad: web_sys::Gamepad) -> GamepadShared {
        let index = gamepad.index();
        let mut gamepads = self.0.gamepads.borrow_mut();
        if gamepads.contains_key(&index) {
            gamepads.insert(index, GamepadShared(Rc::new(gamepad)));
        }
        self.get(&index).expect("[register] Gamepad expected")
    }

    pub fn get(&self, index: &u32) -> Option<GamepadShared> {
        self.0.gamepads.borrow().get(index).map(|g| g.clone())
    }
}

impl Clone for GamepadManagerShared {
    fn clone(&self) -> Self {
        GamepadManagerShared(self.0.clone())
    }
}

impl GamepadShared {
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

impl Clone for GamepadShared {
    fn clone(&self) -> Self {
        GamepadShared(self.0.clone())
    }
}
