use super::utils;
use crate::event::device;
use crate::platform_impl::platform::{backend, device::gamepad, GamepadHandle};
use std::{cell::RefCell, rc::Rc};

pub struct Shared(Rc<GamepadManager>);

impl Shared {
    pub fn new() -> Shared {
        Shared(Rc::new(GamepadManager {
            gamepads: RefCell::new(Vec::new()),
            events: RefCell::new(Vec::new()),
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

pub struct GamepadManager {
    pub(crate) gamepads: RefCell<Vec<backend::gamepad::Gamepad>>,
    pub(crate) events: RefCell<Vec<(backend::gamepad::Gamepad, device::GamepadEvent)>>,
}

impl GamepadManager {
    // Register every new added/removed gamepad
    pub fn register(&self, gamepad: backend::gamepad::Gamepad) -> backend::gamepad::Gamepad {
        let mut gamepads = self.gamepads.borrow_mut();
        if !gamepads.contains(&gamepad) {
            gamepads.push(gamepad.clone());
        }
        gamepad
    }

    // Called by EventLoop::gamepads()
    pub fn collect_handles(&self) -> Vec<crate::event::device::GamepadHandle> {
        let gamepads = self.gamepads.borrow();

        gamepads
            .iter()
            .map(|gamepad| {
                device::GamepadHandle(GamepadHandle {
                    id: gamepad.index,
                    gamepad: gamepad::Shared::Raw(gamepad.clone()),
                })
            })
            .collect::<Vec<_>>()
    }

    // Get an updated raw gamepad and generate a new mapping
    pub fn collect_changed(&self) -> Vec<backend::gamepad::Gamepad> {
        let gamepads = self.gamepads.borrow();

        gamepads
            .iter()
            .map(|g| backend::gamepad::Gamepad::new(g.raw()))
            .collect()
    }

    // Collect gamepad events (buttons/axes/sticks)
    // dispatch to handler and update gamepads
    pub fn collect_events<F>(&self, mut handler: F)
    where
        F: 'static + FnMut((device::GamepadHandle, device::GamepadEvent)),
    {
        let mut events = self.events.borrow_mut();
        let old_gamepads = self.gamepads.borrow().clone();
        let new_gamepads = self.collect_changed();

        // Collect events
        match (old_gamepads.get(0), new_gamepads.get(0)) {
            (Some(old), Some(new)) => {
                // Button events
                let buttons = old.mapping.buttons().zip(new.mapping.buttons()).enumerate();
                for (btn_index, (old_button, new_button)) in buttons {
                    match (old_button, new_button) {
                        (false, true) => {
                            events.push((new.clone(), utils::gamepad_button(btn_index, true)))
                        }
                        (true, false) => {
                            events.push((new.clone(), utils::gamepad_button(btn_index, false)))
                        }
                        _ => (),
                    }
                }

                // Axis events
                let axes = old.mapping.axes().zip(new.mapping.axes()).enumerate();
                for (axis_index, (old_axis, new_axis)) in axes {
                    if old_axis != new_axis {
                        events.push((new.clone(), utils::gamepad_axis(axis_index, new_axis)))
                    }
                }

                // Stick events
                let mut old_axes = old.mapping.axes();
                let mut new_axes = new.mapping.axes();

                let old_left = (old_axes.next(), old_axes.next());
                let new_left = (new_axes.next(), new_axes.next());
                if old_left != new_left {
                    if let (Some(x), Some(y)) = (new_left.0, new_left.1) {
                        events.push((
                            new.clone(),
                            utils::gamepad_stick(0, 1, x, y, device::Side::Left),
                        ));
                    }
                }

                let old_right = (old_axes.next(), old_axes.next());
                let new_right = (new_axes.next(), new_axes.next());
                if old_right != new_right {
                    if let (Some(x), Some(y)) = (new_right.0, new_right.1) {
                        events.push((
                            new.clone(),
                            utils::gamepad_stick(2, 3, x, y, device::Side::Right),
                        ));
                    }
                }
            }
            _ => {}
        }

        // Dispatch events and drain events vec
        loop {
            if let Some((gamepad, event)) = events.pop() {
                handler((
                    device::GamepadHandle(GamepadHandle {
                        id: gamepad.index,
                        gamepad: gamepad::Shared::Raw(gamepad),
                    }),
                    event,
                ));
            } else {
                break;
            }
        }

        // Update gamepads
        self.gamepads.replace(new_gamepads);
    }
}
