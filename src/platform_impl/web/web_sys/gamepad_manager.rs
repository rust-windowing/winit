use super::event;
use super::gamepad;
use crate::event::device;
use crate::platform_impl::platform::device::gamepad::{native_ev_codes, EventCode};
use std::{cell::RefCell, rc::Rc};

pub struct Shared(Rc<GamepadManager>);

pub struct GamepadManager {
    gamepads: RefCell<Vec<gamepad::Gamepad>>,
}

impl Shared {
    pub fn create() -> Shared {
        Shared(Rc::new(GamepadManager {
            gamepads: RefCell::new(Vec::new()),
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
    pub fn register(&self, gamepad: web_sys::Gamepad) -> gamepad::Gamepad {
        let mut gamepads = self.gamepads.borrow_mut();
        let w = gamepad::Gamepad::new(gamepad);
        if !gamepads.contains(&w) {
            gamepads.push(w.clone());
        }
        w
    }

    pub fn collect_new(&self) -> Vec<gamepad::Gamepad> {
        let gamepads = self.gamepads.borrow();

        gamepads
            .iter()
            .map(|g| gamepad::Gamepad::new(g.raw()))
            .collect()
    }

    pub fn collect_events(&self, events: &mut Vec<(gamepad::Gamepad, device::GamepadEvent)>) {
        let old_gamepads = self.gamepads.borrow().clone();
        let new_gamepads = self.collect_new();

        match (old_gamepads.get(0), new_gamepads.get(0)) {
            (Some(old), Some(new)) => {
                let buttons = old.mapping.buttons().zip(new.mapping.buttons()).enumerate();
                for (btn_index, (old_button, new_button)) in buttons {
                    let code = native_ev_codes::button_code(btn_index);
                    match (old_button, new_button) {
                        (false, true) => {
                            events.push((new.clone(), event::gamepad_button(code, true)))
                        }
                        (true, false) => {
                            events.push((new.clone(), event::gamepad_button(code, false)))
                        }
                        _ => (),
                    }
                }
            }
            _ => {}
        }

        self.gamepads.replace(new_gamepads);
        // super::log(&format!("{:?}", events).to_string());
    }
}
