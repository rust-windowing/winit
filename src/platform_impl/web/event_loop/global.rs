use super::super::device::{gamepad, GamepadHandle};
use super::backend;
use crate::event::device;
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
pub struct Window {
    raw: RefCell<Option<backend::window::Shared>>,
    gamepads: Rc<RefCell<Vec<backend::gamepad::Gamepad>>>,
}

#[derive(Debug)]
pub struct Shared(Rc<Window>);

impl Shared {
    pub fn new() -> Self {
        Self(Rc::new(Window {
            raw: RefCell::new(None),
            gamepads: Rc::new(RefCell::new(Vec::new())),
        }))
    }

    // Request window object and listen global events
    pub fn register_events(&self) -> Result<(), crate::error::OsError> {
        if (*self.0.raw.borrow()).is_none() {
            let shared = backend::window::Shared::create()?;
            let mut window = shared.0.borrow_mut();

            let shared_gamepads = self.0.gamepads.clone();
            window.on_gamepad_connected(move |gamepad: backend::gamepad::Gamepad| {
                let mut gamepads = shared_gamepads.borrow_mut();
                if !gamepads.contains(&gamepad) {
                    gamepads.push(gamepad);
                }
            });

            let shared_gamepads = self.0.gamepads.clone();
            window.on_gamepad_disconnected(move |gamepad: backend::gamepad::Gamepad| {
                let mut gamepads = shared_gamepads.borrow_mut();
                let index = gamepads.iter().position(|g| g.index() == gamepad.index());
                if let Some(index) = index {
                    gamepads.remove(index);
                }
            });

            self.0.raw.replace(Some(shared.clone()));
        }

        Ok(())
    }

    // Google Chrome create an array of [null, null, null, null].
    // To fix that issue, I create my own list of gamepads
    // by listening "gamepadconnected" and "gamepaddisconnected"
    pub fn get_gamepads(&self) -> Vec<backend::gamepad::Gamepad> {
        let gamepads = self.0.gamepads.borrow_mut();

        gamepads
            .iter()
            .map(|g| {
                let mut c = g.clone();
                c.remap();
                c
            })
            .collect()
    }

    // Return gamepads handles required for EventLoop::gamepads()
    pub fn get_gamepad_handles(&self) -> Vec<device::GamepadHandle> {
        self.get_gamepads()
            .iter()
            .map(|gamepad| {
                device::GamepadHandle(GamepadHandle {
                    id: gamepad.index,
                    gamepad: gamepad::Shared::Raw(gamepad.clone()),
                })
            })
            .collect()
    }
}

impl Clone for Shared {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}
