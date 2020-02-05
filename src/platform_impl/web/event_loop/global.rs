use super::{backend, runner};
use crate::event::{device, Event};
use crate::platform_impl::platform::device::{gamepad, GamepadHandle};
use std::cell::RefCell;

pub struct Emitter {
    window: RefCell<Option<backend::window::Shared>>,
    gamepad_events: RefCell<Vec<(backend::gamepad::Gamepad, device::GamepadEvent)>>,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            window: RefCell::new(None),
            gamepad_events: RefCell::new(Vec::new()),
        }
    }

    pub fn register_events<T: 'static>(
        &self,
        runner: &runner::Shared<T>,
    ) -> Result<(), crate::error::OsError> {
        if (*self.window.borrow()).is_none() {
            let shared = backend::window::Shared::create()?;
            let mut window = shared.0.borrow_mut();

            let r = runner.clone();
            window.on_gamepad_connected(move |gamepad: backend::gamepad::Gamepad| {
                r.send_event(Event::GamepadEvent(
                    device::GamepadHandle(GamepadHandle {
                        id: gamepad.index() as i32,
                        gamepad: gamepad::Shared::Raw(gamepad),
                    }),
                    device::GamepadEvent::Added,
                ));
            });

            let r = runner.clone();
            window.on_gamepad_disconnected(move |gamepad: backend::gamepad::Gamepad| {
                r.send_event(Event::GamepadEvent(
                    device::GamepadHandle(GamepadHandle {
                        id: gamepad.index() as i32,
                        gamepad: gamepad::Shared::Raw(gamepad),
                    }),
                    device::GamepadEvent::Removed,
                ));
            });

            self.window.replace(Some(shared.clone()));
        }

        Ok(())
    }

    pub fn collect_gamepad_events(&self) -> Vec<(device::GamepadHandle, device::GamepadEvent)> {
        let mut gamepad_events = self.gamepad_events.borrow_mut();
        match *self.window.borrow() {
            Some(ref shared) => {
                let window = shared.0.borrow();
                window.collect_gamepad_events(&mut gamepad_events);
            }
            None => (),
        }

        gamepad_events
            .drain(..)
            .map(|(gamepad, event)| {
                (
                    device::GamepadHandle(GamepadHandle {
                        id: gamepad.index() as i32,
                        gamepad: gamepad::Shared::Raw(gamepad),
                    }),
                    event,
                )
            })
            .collect()
    }
}
