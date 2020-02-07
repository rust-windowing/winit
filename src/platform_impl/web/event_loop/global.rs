use super::{backend, runner};
use crate::event::{device, Event};
use crate::platform_impl::platform::device::{gamepad, gamepad_manager, GamepadHandle};
use std::cell::RefCell;

// Global emitter for every window.addEventListener
pub struct Emitter {
    window: RefCell<Option<backend::window::Shared>>,
    gamepad_manager: gamepad_manager::Shared,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            window: RefCell::new(None),
            gamepad_manager: gamepad_manager::Shared::new(),
        }
    }

    // Request window object and listen global events
    pub fn register_events<T: 'static>(
        &self,
        runner: &runner::Shared<T>,
    ) -> Result<(), crate::error::OsError> {
        if (*self.window.borrow()).is_none() {
            let shared = backend::window::Shared::create()?;
            let mut window = shared.0.borrow_mut();

            let manager = self.gamepad_manager.clone().manager();
            let r = runner.clone();
            window.on_gamepad_connected(move |gamepad: backend::gamepad::Gamepad| {
                let gamepad = manager.register(gamepad);
                r.send_event(Event::GamepadEvent(
                    device::GamepadHandle(GamepadHandle {
                        id: gamepad.index,
                        gamepad: gamepad::Shared::Raw(gamepad),
                    }),
                    device::GamepadEvent::Added,
                ));
            });

            let manager = self.gamepad_manager.clone().manager();
            let r = runner.clone();
            window.on_gamepad_disconnected(move |gamepad: backend::gamepad::Gamepad| {
                let gamepad = manager.register(gamepad);
                r.send_event(Event::GamepadEvent(
                    device::GamepadHandle(GamepadHandle {
                        id: gamepad.index,
                        gamepad: gamepad::Shared::Raw(gamepad),
                    }),
                    device::GamepadEvent::Removed,
                ));
            });

            self.window.replace(Some(shared.clone()));
        }

        Ok(())
    }

    // Collect and dispatch gamepad events
    pub fn collect_gamepad_events<F>(&self, handler: F)
    where
        F: 'static + FnMut((device::GamepadHandle, device::GamepadEvent)),
    {
        self.gamepad_manager.manager().collect_events(handler);
    }

    // Collect gamepad handles
    pub fn collect_gamepad_handles(&self) -> Vec<crate::event::device::GamepadHandle> {
        self.gamepad_manager.manager().collect_handles()
    }
}
