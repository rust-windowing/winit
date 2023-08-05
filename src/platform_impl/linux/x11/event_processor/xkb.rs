//! Handles events from the `xkb` extension.

use super::prelude::*;

use x11rb::protocol::xkb;

use crate::keyboard::ModifiersState;

impl EventProcessor {
    /// Handle the `NewKeyboardNotify` event.
    fn handle_new_keyboard_notify(
        &mut self,
        wt: &WindowTarget,
        xev: xkb::NewKeyboardNotifyEvent,
        _callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time as xproto::Timestamp);

        let keycodes_changed = xev.changed.contains(xkb::NKNDetail::KEYCODES);
        let geometry_changed = xev.changed.contains(xkb::NKNDetail::GEOMETRY);

        if xev.device_id as i32 == self.kb_state.core_keyboard_id
            && (keycodes_changed || geometry_changed)
        {
            unsafe { self.kb_state.init_with_x11_keymap() };
        }
    }

    /// Handle the `StateNotify` event.
    fn handle_state_notify(
        &mut self,
        wt: &WindowTarget,
        xev: xkb::StateNotifyEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time);

        let prev_mods = self.kb_state.mods_state();
        self.kb_state.update_modifiers(
            xev.base_mods.into(),
            xev.latched_mods.into(),
            xev.locked_mods.into(),
            xev.base_group as u32,
            xev.latched_group as u32,
            xev.locked_group.into(),
        );
        let new_mods = self.kb_state.mods_state();
        if prev_mods != new_mods {
            if let Some(window) = self.active_window {
                callback(Event::WindowEvent {
                    window_id: mkwid(window),
                    event: WindowEvent::ModifiersChanged(
                        Into::<ModifiersState>::into(new_mods).into(),
                    ),
                });
            }
        }
    }
}

event_handlers! {
    xkb_code(xkb::NEW_KEYBOARD_NOTIFY_EVENT) => EventProcessor::handle_new_keyboard_notify,
    xkb_code(xkb::STATE_NOTIFY_EVENT) => EventProcessor::handle_state_notify,
}
