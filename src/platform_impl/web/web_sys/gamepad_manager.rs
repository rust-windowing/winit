use super::gamepad;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct Shared(Rc<GamepadManager>);

pub struct GamepadManager {
    gamepads: RefCell<HashMap<u32, gamepad::Shared>>,
}

impl Shared {
    pub fn create() -> Shared {
        Shared(Rc::new(GamepadManager {
            gamepads: RefCell::new(HashMap::new()),
        }))
    }

    pub fn manager(&self) -> Rc<GamepadManager> {
        self.0.clone()
    }
}

impl Clone for Shared {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl GamepadManager {
    pub fn register(&self, gamepad: web_sys::Gamepad) -> gamepad::Shared {
        let index = gamepad.index();
        let mut gamepads = self.gamepads.borrow_mut();
        if !gamepads.contains_key(&index) {
            gamepads.insert(index, gamepad::Shared(Rc::new(gamepad)));
        }
        gamepads
            .get(&index)
            .map(|g| g.clone())
            .expect("[register] Gamepad expected")
    }

    pub fn get(&self, index: &u32) -> Option<gamepad::Shared> {
        self.gamepads.borrow().get(index).map(|g| g.clone())
    }
}
