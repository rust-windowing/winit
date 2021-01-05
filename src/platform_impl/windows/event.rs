use std::{
    char,
    os::raw::c_int,
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};

use crate::keyboard::{ModifiersState, Key};

use winapi::{
    shared::minwindef::{HKL, HKL__},
    um::winuser,
};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct KeyEventExtra {
    pub char_with_all_modifers: Option<&'static str>,
    pub key_without_modifers: Key<'static>,
}

fn key_pressed(vkey: c_int) -> bool {
    unsafe { (winuser::GetKeyState(vkey) & (1 << 15)) == (1 << 15) }
}

pub fn get_key_mods() -> ModifiersState {
    let filter_out_altgr = layout_uses_altgr() && key_pressed(winuser::VK_RMENU);

    let mut mods = ModifiersState::empty();
    mods.set(ModifiersState::SHIFT, key_pressed(winuser::VK_SHIFT));
    mods.set(
        ModifiersState::CONTROL,
        key_pressed(winuser::VK_CONTROL) && !filter_out_altgr,
    );
    mods.set(
        ModifiersState::ALT,
        key_pressed(winuser::VK_MENU) && !filter_out_altgr,
    );
    mods.set(
        ModifiersState::META,
        key_pressed(winuser::VK_LWIN) || key_pressed(winuser::VK_RWIN),
    );
    mods
}

bitflags! {
    #[derive(Default)]
    pub struct ModifiersStateSide: u32 {
        const LSHIFT = 0b010 << 0;
        const RSHIFT = 0b001 << 0;

        const LCTRL = 0b010 << 3;
        const RCTRL = 0b001 << 3;

        const LALT = 0b010 << 6;
        const RALT = 0b001 << 6;

        const LLOGO = 0b010 << 9;
        const RLOGO = 0b001 << 9;
    }
}

impl ModifiersStateSide {
    pub fn filter_out_altgr(&self) -> ModifiersStateSide {
        match layout_uses_altgr() && self.contains(Self::RALT) {
            false => *self,
            true => *self & !(Self::LCTRL | Self::RCTRL | Self::LALT | Self::RALT),
        }
    }
}

impl From<ModifiersStateSide> for ModifiersState {
    fn from(side: ModifiersStateSide) -> Self {
        let mut state = ModifiersState::default();
        state.set(
            Self::SHIFT,
            side.intersects(ModifiersStateSide::LSHIFT | ModifiersStateSide::RSHIFT),
        );
        state.set(
            Self::CONTROL,
            side.intersects(ModifiersStateSide::LCTRL | ModifiersStateSide::RCTRL),
        );
        state.set(
            Self::ALT,
            side.intersects(ModifiersStateSide::LALT | ModifiersStateSide::RALT),
        );
        state.set(
            Self::META,
            side.intersects(ModifiersStateSide::LLOGO | ModifiersStateSide::RLOGO),
        );
        state
    }
}

unsafe fn get_char(keyboard_state: &[u8; 256], v_key: u32, hkl: HKL) -> Option<char> {
    let mut unicode_bytes = [0u16; 5];
    let len = winuser::ToUnicodeEx(
        v_key,
        0,
        keyboard_state.as_ptr(),
        unicode_bytes.as_mut_ptr(),
        unicode_bytes.len() as _,
        0,
        hkl,
    );
    if len >= 1 {
        char::decode_utf16(unicode_bytes.iter().cloned())
            .next()
            .and_then(|c| c.ok())
    } else {
        None
    }
}

/// Figures out if the keyboard layout has an AltGr key instead of an Alt key.
///
/// Unfortunately, the Windows API doesn't give a way for us to conveniently figure that out. So,
/// we use a technique blatantly stolen from [the Firefox source code][source]: iterate over every
/// possible virtual key and compare the `char` output when AltGr is pressed vs when it isn't. If
/// pressing AltGr outputs characters that are different from the standard characters, the layout
/// uses AltGr. Otherwise, it doesn't.
///
/// [source]: https://github.com/mozilla/gecko-dev/blob/265e6721798a455604328ed5262f430cfcc37c2f/widget/windows/KeyboardLayout.cpp#L4356-L4416
fn layout_uses_altgr() -> bool {
    unsafe {
        static ACTIVE_LAYOUT: AtomicPtr<HKL__> = AtomicPtr::new(ptr::null_mut());
        static USES_ALTGR: AtomicBool = AtomicBool::new(false);

        let hkl = winuser::GetKeyboardLayout(0);
        let old_hkl = ACTIVE_LAYOUT.swap(hkl, Ordering::SeqCst);

        if hkl == old_hkl {
            return USES_ALTGR.load(Ordering::SeqCst);
        }

        let mut keyboard_state_altgr = [0u8; 256];
        // AltGr is an alias for Ctrl+Alt for... some reason. Whatever it is, those are the keypresses
        // we have to emulate to do an AltGr test.
        keyboard_state_altgr[winuser::VK_MENU as usize] = 0x80;
        keyboard_state_altgr[winuser::VK_CONTROL as usize] = 0x80;

        let keyboard_state_empty = [0u8; 256];

        for v_key in 0..=255 {
            let key_noaltgr = get_char(&keyboard_state_empty, v_key, hkl);
            let key_altgr = get_char(&keyboard_state_altgr, v_key, hkl);
            if let (Some(noaltgr), Some(altgr)) = (key_noaltgr, key_altgr) {
                if noaltgr != altgr {
                    USES_ALTGR.store(true, Ordering::SeqCst);
                    return true;
                }
            }
        }

        USES_ALTGR.store(false, Ordering::SeqCst);
        false
    }
}
