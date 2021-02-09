use super::utils;
use crate::event::device;
use crate::platform_impl::platform::{backend, device::gamepad, event_loop::global, GamepadHandle};
use std::collections::VecDeque;

pub struct Manager {
    pub(crate) gamepads: Vec<backend::gamepad::Gamepad>,
    pub(crate) events: VecDeque<(backend::gamepad::Gamepad, device::GamepadEvent)>,
    pub(crate) global_window: Option<global::Shared>,
}

impl Manager {
    pub fn new() -> Self {
        Self {
            gamepads: Vec::new(),
            events: VecDeque::new(),
            global_window: None,
        }
    }

    // Register global window to fetch gamepads.
    // Due to Chrome issue, I prefer to use its gamepad list
    pub fn set_global_window(&mut self, global_window: global::Shared) {
        self.global_window.replace(global_window);
    }

    // Get an updated raw gamepad and generate a new mapping
    pub fn collect_gamepads(&self) -> Option<Vec<backend::gamepad::Gamepad>> {
        self.global_window.as_ref().map(|w| w.get_gamepads())
    }

    // Collect gamepad events (buttons/axes/sticks)
    // dispatch to handler and update gamepads
    pub fn collect_events<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut((device::GamepadHandle, device::GamepadEvent)),
    {
        let opt_new_gamepads = self.collect_gamepads();
        if opt_new_gamepads.is_none() {
            return;
        }

        let new_gamepads = opt_new_gamepads.unwrap();
        let old_gamepads = &self.gamepads;

        let mut old_index = 0;
        let mut new_index = 0;

        // Collect events
        loop {
            match (old_gamepads.get(old_index), new_gamepads.get(new_index)) {
                (Some(old), Some(new)) if old.index() == new.index() => {
                    // Button events
                    let buttons = old.mapping.buttons().zip(new.mapping.buttons()).enumerate();
                    for (btn_index, (old_button, new_button)) in buttons {
                        match (old_button, new_button) {
                            (false, true) => self
                                .events
                                .push_back((new.clone(), utils::gamepad_button(btn_index, true))),
                            (true, false) => self
                                .events
                                .push_back((new.clone(), utils::gamepad_button(btn_index, false))),
                            _ => (),
                        }
                    }

                    // Axis events
                    let axes = old.mapping.axes().zip(new.mapping.axes()).enumerate();
                    for (axis_index, (old_axis, new_axis)) in axes {
                        if old_axis != new_axis {
                            self.events
                                .push_back((new.clone(), utils::gamepad_axis(axis_index, new_axis)))
                        }
                    }

                    // Stick events
                    let mut old_axes = old.mapping.axes();
                    let mut new_axes = new.mapping.axes();

                    let old_left = (old_axes.next(), old_axes.next());
                    let new_left = (new_axes.next(), new_axes.next());
                    if old_left != new_left {
                        if let (Some(x), Some(y)) = (new_left.0, new_left.1) {
                            self.events.push_back((
                                new.clone(),
                                utils::gamepad_stick(0, 1, x, y, device::Side::Left),
                            ));
                        }
                    }

                    let old_right = (old_axes.next(), old_axes.next());
                    let new_right = (new_axes.next(), new_axes.next());
                    if old_right != new_right {
                        if let (Some(x), Some(y)) = (new_right.0, new_right.1) {
                            self.events.push_back((
                                new.clone(),
                                utils::gamepad_stick(2, 3, x, y, device::Side::Right),
                            ));
                        }
                    }

                    // Increment indices
                    old_index += 1;
                    new_index += 1;
                }

                // Connect
                (None, Some(new)) => {
                    self.events
                        .push_back((new.clone(), device::GamepadEvent::Added));
                    new_index += 1;
                }

                // Connect
                (Some(old), Some(new)) if old.index > new.index => {
                    self.events
                        .push_back((new.clone(), device::GamepadEvent::Added));
                    new_index += 1;
                }

                // Disconnect
                (Some(old), Some(_new)) => {
                    self.events
                        .push_back((old.clone(), device::GamepadEvent::Removed));
                    old_index += 1;
                }

                // Disconnect
                (Some(old), None) => {
                    self.events
                        .push_back((old.clone(), device::GamepadEvent::Removed));
                    old_index += 1;
                }

                // Break loop
                (None, None) => break,
            }
        }

        // Dispatch events and drain events vec
        loop {
            if let Some((gamepad, event)) = self.events.pop_front() {
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
        self.gamepads = new_gamepads;
    }
}
