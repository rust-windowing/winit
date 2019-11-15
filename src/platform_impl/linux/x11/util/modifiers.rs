use std::{collections::HashMap, slice};

use super::*;

use crate::event::{ElementState, ModifiersState};

// Offsets within XModifierKeymap to each set of keycodes.
// We are only interested in Shift, Control, Alt, and Logo.
//
// There are 8 sets total. The order of keycode sets is:
//     Shift, Lock, Control, Mod1 (Alt), Mod2, Mod3, Mod4 (Logo), Mod5
//
// https://tronche.com/gui/x/xlib/input/XSetModifierMapping.html
const SHIFT_OFFSET: usize = 0;
const CONTROL_OFFSET: usize = 2;
const ALT_OFFSET: usize = 3;
const LOGO_OFFSET: usize = 6;
const NUM_MODS: usize = 8;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Modifier {
    Alt,
    Ctrl,
    Shift,
    Logo,
}

#[derive(Debug, Default)]
pub struct ModifierKeymap {
    // Maps keycodes to modifiers
    keys: HashMap<ffi::KeyCode, Modifier>,
}

#[derive(Clone, Debug, Default)]
pub struct ModifierKeyState {
    // Contains currently pressed modifier keys and their corresponding modifiers
    keys: HashMap<ffi::KeyCode, Modifier>,
    state: ModifiersState,
}

impl ModifierKeymap {
    pub fn new() -> ModifierKeymap {
        ModifierKeymap::default()
    }

    pub fn get_modifier(&self, keycode: ffi::KeyCode) -> Option<Modifier> {
        self.keys.get(&keycode).cloned()
    }

    pub fn reset_from_x_connection(&mut self, xconn: &XConnection) {
        unsafe {
            let keymap = (xconn.xlib.XGetModifierMapping)(xconn.display);

            if keymap.is_null() {
                panic!("failed to allocate XModifierKeymap");
            }

            self.reset_from_x_keymap(&*keymap);

            (xconn.xlib.XFreeModifiermap)(keymap);
        }
    }

    pub fn reset_from_x_keymap(&mut self, keymap: &ffi::XModifierKeymap) {
        let keys_per_mod = keymap.max_keypermod as usize;

        let keys = unsafe {
            slice::from_raw_parts(keymap.modifiermap as *const _, keys_per_mod * NUM_MODS)
        };

        self.keys.clear();

        self.read_x_keys(keys, SHIFT_OFFSET, keys_per_mod, Modifier::Shift);
        self.read_x_keys(keys, CONTROL_OFFSET, keys_per_mod, Modifier::Ctrl);
        self.read_x_keys(keys, ALT_OFFSET, keys_per_mod, Modifier::Alt);
        self.read_x_keys(keys, LOGO_OFFSET, keys_per_mod, Modifier::Logo);
    }

    fn read_x_keys(
        &mut self,
        keys: &[ffi::KeyCode],
        offset: usize,
        keys_per_mod: usize,
        modifier: Modifier,
    ) {
        let start = offset * keys_per_mod;
        let end = start + keys_per_mod;

        for &keycode in &keys[start..end] {
            if keycode != 0 {
                self.keys.insert(keycode, modifier);
            }
        }
    }
}

impl ModifierKeyState {
    pub fn update_keymap(&mut self, mods: &ModifierKeymap) {
        self.keys.retain(|k, v| {
            if let Some(m) = mods.get_modifier(*k) {
                *v = m;
                true
            } else {
                false
            }
        });

        self.reset_state();
    }

    pub fn update_state(
        &mut self,
        state: &ModifiersState,
        except: Option<Modifier>,
    ) -> Option<ModifiersState> {
        let mut state = *state;

        match except {
            Some(Modifier::Alt) => state.alt = self.state.alt,
            Some(Modifier::Ctrl) => state.ctrl = self.state.ctrl,
            Some(Modifier::Shift) => state.shift = self.state.shift,
            Some(Modifier::Logo) => state.logo = self.state.logo,
            None => (),
        }

        if self.state == state {
            None
        } else {
            self.state = state;
            Some(state)
        }
    }

    pub fn modifiers(&self) -> ModifiersState {
        self.state
    }

    pub fn key_event(&mut self, state: ElementState, keycode: ffi::KeyCode, modifier: Modifier) {
        match state {
            ElementState::Pressed => self.key_press(keycode, modifier),
            ElementState::Released => self.key_release(keycode),
        }
    }

    pub fn key_press(&mut self, keycode: ffi::KeyCode, modifier: Modifier) {
        self.keys.insert(keycode, modifier);

        set_modifier(&mut self.state, modifier);
    }

    pub fn key_release(&mut self, keycode: ffi::KeyCode) {
        if let Some(modifier) = self.keys.remove(&keycode) {
            if self.keys.values().find(|&&m| m == modifier).is_none() {
                unset_modifier(&mut self.state, modifier);
            }
        }
    }

    fn reset_state(&mut self) {
        let mut state = ModifiersState::default();

        for &m in self.keys.values() {
            set_modifier(&mut state, m);
        }

        self.state = state;
    }
}

fn unset_modifier(state: &mut ModifiersState, modifier: Modifier) {
    match modifier {
        Modifier::Alt => state.alt = false,
        Modifier::Ctrl => state.ctrl = false,
        Modifier::Shift => state.shift = false,
        Modifier::Logo => state.logo = false,
    }
}

fn set_modifier(state: &mut ModifiersState, modifier: Modifier) {
    match modifier {
        Modifier::Alt => state.alt = true,
        Modifier::Ctrl => state.ctrl = true,
        Modifier::Shift => state.shift = true,
        Modifier::Logo => state.logo = true,
    }
}
