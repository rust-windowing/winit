use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct GamepadManagerShared(Rc<GamepadManager>);

pub struct GamepadManager {
    gamepads: RefCell<HashMap<u32, Gamepad>>,
}

pub struct Gamepad(Option<Rc<web_sys::Gamepad>>);

impl GamepadManagerShared {
    pub fn create() -> GamepadManagerShared {
        GamepadManagerShared(Rc::new(GamepadManager {
            gamepads: RefCell::new(HashMap::new()),
        }))
    }

    pub fn register(&self, gamepad: web_sys::Gamepad) -> u32 {
        let index = gamepad.index();
        let mut gamepads = self.0.gamepads.borrow_mut();
        if gamepads.contains_key(&index) {
            gamepads.insert(index, Gamepad(Some(Rc::new(gamepad))));
        }
        index
    }

    pub fn get(&self, index: &u32) -> Option<Gamepad> {
        self.0.gamepads.borrow().get(index).map(|g| g.clone())
    }

    pub fn is_present(&self, index: &u32) -> bool {
        self.0.gamepads.borrow().contains_key(index)
    }
}

impl Clone for GamepadManagerShared {
    fn clone(&self) -> Self {
        GamepadManagerShared(self.0.clone())
    }
}

impl Default for GamepadManagerShared {
    fn default() -> Self {
        Self(Rc::new(GamepadManager {
            gamepads: RefCell::new(HashMap::new()),
        }))
    }
}

impl Gamepad {
    pub fn id(&self) -> String {
        match &self.0 {
            Some(g) => g.id(),
            None => String::new(),
        }
    }

    pub fn index(&self) -> i32 {
        match &self.0 {
            Some(g) => g.index() as i32,
            None => -1,
        }
    }

    pub fn connected(&self) -> bool {
        match &self.0 {
            Some(g) => g.connected(),
            None => false,
        }
    }

    // pub fn vibrate(&self, value: f64, duration: f64) {
    //     if let Some(g) = self.0.inner {
    //         for actuator in g.haptic_actuators().values() {
    //             actuator
    //             .ok()
    //             .and_then(|a| match a.type_ {
    //                 web_sys::GamepadHapticActuatorType::Vibration => {
    //                     a.pulse(value, duration);
    //                     Some(())
    //                 },
    //                 _ => None,
    //             });
    //         }
    //     }
    // }
}

impl Clone for Gamepad {
    fn clone(&self) -> Self {
        match &self.0 {
            Some(g) => Gamepad(Some(g.clone())),
            None => Gamepad(None),
        }
    }
}

impl Default for Gamepad {
    fn default() -> Self {
        Self(None)
    }
}
