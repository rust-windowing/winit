//! Handling of various keyboard events.

use sctk::reexports::client::protocol::wl_keyboard::KeyState;

use sctk::seat::keyboard::Event as KeyboardEvent;

use crate::event::{ElementState, KeyboardInput, ModifiersState, WindowEvent};
use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::{self, DeviceId};

use super::keymap;
use super::KeyboardInner;

#[inline]
pub(super) fn handle_keyboard(
    event: KeyboardEvent<'_>,
    inner: &mut KeyboardInner,
    winit_state: &mut WinitState,
) {
    let event_sink = &mut winit_state.event_sink;
    match event {
        KeyboardEvent::Enter { surface, .. } => {
            let window_id = wayland::make_wid(&surface);

            // Window gained focus.
            event_sink.push_window_event(WindowEvent::Focused(true), window_id);

            // Dispatch modifers changes that we've received before getting `Enter` event.
            if let Some(modifiers) = inner.pending_modifers_state.take() {
                *inner.modifiers_state.borrow_mut() = modifiers;
                event_sink.push_window_event(WindowEvent::ModifiersChanged(modifiers), window_id);
            }

            inner.target_window_id = Some(window_id);
        }
        KeyboardEvent::Leave { surface, .. } => {
            let window_id = wayland::make_wid(&surface);

            // Notify that no modifiers are being pressed.
            if !inner.modifiers_state.borrow().is_empty() {
                event_sink.push_window_event(
                    WindowEvent::ModifiersChanged(ModifiersState::empty()),
                    window_id,
                );
            }

            // Window lost focus.
            event_sink.push_window_event(WindowEvent::Focused(false), window_id);

            // Reset the id.
            inner.target_window_id = None;
        }
        KeyboardEvent::Key {
            rawkey,
            keysym,
            state,
            utf8,
            ..
        } => {
            let window_id = match inner.target_window_id {
                Some(window_id) => window_id,
                None => return,
            };

            let state = match state {
                KeyState::Pressed => ElementState::Pressed,
                KeyState::Released => ElementState::Released,
                _ => unreachable!(),
            };

            let virtual_keycode = keymap::keysym_to_vkey(keysym);

            event_sink.push_window_event(
                #[allow(deprecated)]
                WindowEvent::KeyboardInput {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    input: KeyboardInput {
                        state,
                        scancode: rawkey,
                        virtual_keycode,
                        modifiers: *inner.modifiers_state.borrow(),
                    },
                    is_synthetic: false,
                },
                window_id,
            );

            // Send ReceivedCharacter event only on ElementState::Pressed.
            if ElementState::Released == state {
                return;
            }

            if let Some(txt) = utf8 {
                for ch in txt.chars() {
                    event_sink.push_window_event(WindowEvent::ReceivedCharacter(ch), window_id);
                }
            }
        }
        KeyboardEvent::Repeat {
            rawkey,
            keysym,
            utf8,
            ..
        } => {
            let window_id = match inner.target_window_id {
                Some(window_id) => window_id,
                None => return,
            };

            let virtual_keycode = keymap::keysym_to_vkey(keysym);

            event_sink.push_window_event(
                #[allow(deprecated)]
                WindowEvent::KeyboardInput {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    input: KeyboardInput {
                        state: ElementState::Pressed,
                        scancode: rawkey,
                        virtual_keycode,
                        modifiers: *inner.modifiers_state.borrow(),
                    },
                    is_synthetic: false,
                },
                window_id,
            );

            if let Some(txt) = utf8 {
                for ch in txt.chars() {
                    event_sink.push_window_event(WindowEvent::ReceivedCharacter(ch), window_id);
                }
            }
        }
        KeyboardEvent::Modifiers { modifiers } => {
            let modifiers = ModifiersState::from(modifiers);
            if let Some(window_id) = inner.target_window_id {
                *inner.modifiers_state.borrow_mut() = modifiers;

                event_sink.push_window_event(WindowEvent::ModifiersChanged(modifiers), window_id);
            } else {
                // Compositor must send modifiers after wl_keyboard::enter, however certain
                // compositors are still sending it before, so stash such events and send
                // them on wl_keyboard::enter.
                inner.pending_modifers_state = Some(modifiers);
            }
        }
    }
}
