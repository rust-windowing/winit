//! Handles the `KeyPress` and `KeyRelease`. events

use super::prelude::*;

use crate::event::{ElementState, Ime};

impl EventProcessor {
    /// Handle the `KeyPress` and `KeyRelease` events.
    fn handle_key(
        &mut self,
        wt: &WindowTarget,
        xkev: xproto::KeyPressEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Set the timestamp.
        wt.xconn.set_timestamp(xkev.time as xproto::Timestamp);

        let window = match self.active_window {
            Some(window) => window,
            None => return,
        };

        let window_id = mkwid(window);
        let device_id = mkdid(util::VIRTUAL_CORE_KEYBOARD);

        let keycode = xkev.detail as _;

        // Update state to track key repeats and determine whether this key was a repeat.
        //
        // Note, when a key is held before focusing on this window the first
        // (non-synthetic) event will not be flagged as a repeat (also note that the
        // synthetic press event that is generated before this when the window gains focus
        // will also not be flagged as a repeat).
        //
        // Only keys that can repeat should change the held_key_press state since a
        // continuously held repeatable key may continue repeating after the press of a
        // non-repeatable key.
        let repeat = if self.kb_state.key_repeats(keycode) {
            let is_latest_held = self.held_key_press == Some(keycode);

            if xkev.response_type & 0x7F == xproto::KEY_PRESS_EVENT {
                self.held_key_press = Some(keycode);
                is_latest_held
            } else {
                // Check that the released key is the latest repeatable key that has been
                // pressed, since repeats will continue for the latest key press if a
                // different previously pressed key is released.
                if is_latest_held {
                    self.held_key_press = None;
                }
                false
            }
        } else {
            false
        };

        let state = if xkev.response_type & 0x7F == xproto::KEY_PRESS_EVENT {
            ElementState::Pressed
        } else {
            ElementState::Released
        };

        if keycode != 0 && !self.is_composing {
            let event = self.kb_state.process_key_event(keycode, state, repeat);
            callback(Event::WindowEvent {
                window_id,
                event: WindowEvent::KeyboardInput {
                    device_id,
                    event,
                    is_synthetic: false,
                },
            });
        } else if let Some(ic) = wt.ime.borrow().get_context(window as ffi::Window) {
            let written = wt.xconn.lookup_utf8(ic, &xkev);
            if !written.is_empty() {
                let event = Event::WindowEvent {
                    window_id,
                    event: WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                };
                callback(event);

                let event = Event::WindowEvent {
                    window_id,
                    event: WindowEvent::Ime(Ime::Commit(written)),
                };

                self.is_composing = false;
                callback(event);
            }
        }
    }
}

event_handlers! {
    xp_code(xproto::KEY_PRESS_EVENT) => EventProcessor::handle_key,
    xp_code(xproto::KEY_RELEASE_EVENT) => EventProcessor::handle_key,
}
