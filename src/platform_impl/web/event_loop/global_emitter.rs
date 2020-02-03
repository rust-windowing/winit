use super::{backend, runner};
use crate::event::{device, Event};
use crate::platform_impl::platform::device::{gamepad, GamepadHandle};
use std::{cell::RefCell, rc::Rc};

pub struct Shared(Rc<RefCell<Option<backend::window::Shared>>>);

impl Shared {
    pub fn new() -> Self {
        Shared(Rc::new(RefCell::new(None)))
    }

    pub fn register_events<T: 'static>(
        &self,
        runner: &runner::Shared<T>,
    ) -> Result<(), crate::error::OsError> {
        if (*self.0.borrow()).is_none() {
            let shared = backend::window::Shared::create()?;
            let mut window = shared.0.borrow_mut();

            let r = runner.clone();
            window.on_gamepad_connected(move |gamepad: backend::gamepad::Shared| {
                r.send_event(Event::GamepadEvent(
                    device::GamepadHandle(GamepadHandle {
                        id: gamepad.index() as i32,
                        gamepad: gamepad::Shared::Raw(gamepad),
                    }),
                    device::GamepadEvent::Added,
                ));
            });

            let r = runner.clone();
            window.on_gamepad_disconnected(move |gamepad: backend::gamepad::Shared| {
                r.send_event(Event::GamepadEvent(
                    device::GamepadHandle(GamepadHandle {
                        id: gamepad.index() as i32,
                        gamepad: gamepad::Shared::Raw(gamepad),
                    }),
                    device::GamepadEvent::Removed,
                ));
            });

            self.0.replace(Some(shared.clone()));
        }

        Ok(())
    }
}

impl Clone for Shared {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}
