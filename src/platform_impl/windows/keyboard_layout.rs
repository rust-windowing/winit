
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    sync::Mutex,
    os::windows::ffi::OsStringExt,
};

use lazy_static::lazy_static;


use winapi::{
    shared::{
        minwindef::{HKL, LOWORD, LPARAM, LRESULT, UINT, WPARAM},
    },
    um::{
        winnt::{LANG_JAPANESE, LANG_KOREAN, PRIMARYLANGID},
        winuser,
    },
};

use crate::{
    keyboard::{ModifiersState, Key, KeyCode, NativeKeyCode},
    platform_impl::platform::keyboard::{ExScancode, vkey_to_non_printable, native_key_to_code},
};

lazy_static!{
    pub static ref LAYOUT_CACHE: Mutex<LayoutCache> = Mutex::new(LayoutCache::default());
}

bitflags! {
    pub struct WindowsModifiers : u8 {
        const SHIFT = 1 << 0;
        const CONTROL = 1 << 1;
        const ALT = 1 << 2;
        const CAPS_LOCK = 1 << 3;
        const FLAGS_END = 1 << 4;
    }
}

impl WindowsModifiers {
    pub fn active_modifiers(key_state: &[u8; 256]) -> WindowsModifiers {
        let shift = key_state[winuser::VK_SHIFT as usize] & 0x80 != 0;
        let lshift = key_state[winuser::VK_LSHIFT as usize] & 0x80 != 0;
        let rshift = key_state[winuser::VK_RSHIFT as usize] & 0x80 != 0;
    
        let control = key_state[winuser::VK_CONTROL as usize] & 0x80 != 0;
        let lcontrol = key_state[winuser::VK_LCONTROL as usize] & 0x80 != 0;
        let rcontrol = key_state[winuser::VK_RCONTROL as usize] & 0x80 != 0;
    
        let alt = key_state[winuser::VK_MENU as usize] & 0x80 != 0;
        let lalt = key_state[winuser::VK_LMENU as usize] & 0x80 != 0;
        let ralt = key_state[winuser::VK_RMENU as usize] & 0x80 != 0;
    
        let caps = key_state[winuser::VK_CAPITAL as usize] & 0x01 != 0;
    
        let mut result = WindowsModifiers::empty();
        if shift || lshift || rshift {
            result.insert(WindowsModifiers::SHIFT);
        }
        if control || lcontrol || rcontrol {
            result.insert(WindowsModifiers::CONTROL);
        }
        if alt || lalt || ralt {
            result.insert(WindowsModifiers::ALT);
        }
        if caps {
            result.insert(WindowsModifiers::CAPS_LOCK);
        }
        result
    }

    pub fn apply_to_key_state(self, key_state: &mut [u8; 256]) {
        if self.intersects(Self::SHIFT) {
            key_state[winuser::VK_SHIFT as usize] |= 0x80;
        } else {
            key_state[winuser::VK_SHIFT as usize] &= !0x80;
            key_state[winuser::VK_LSHIFT as usize] &= !0x80;
            key_state[winuser::VK_RSHIFT as usize] &= !0x80;
        }
        if self.intersects(Self::CONTROL) {
            key_state[winuser::VK_CONTROL as usize] |= 0x80;
        } else {
            key_state[winuser::VK_CONTROL as usize] &= !0x80;
            key_state[winuser::VK_LCONTROL as usize] &= !0x80;
            key_state[winuser::VK_RCONTROL as usize] &= !0x80;
        }
        if self.intersects(Self::ALT) {
            key_state[winuser::VK_MENU as usize] |= 0x80;
        } else {
            key_state[winuser::VK_MENU as usize] &= !0x80;
            key_state[winuser::VK_LMENU as usize] &= !0x80;
            key_state[winuser::VK_RMENU as usize] &= !0x80;
        }
        if self.intersects(Self::CAPS_LOCK) {
            key_state[winuser::VK_CAPITAL as usize] |= 0x80;
        } else {
            key_state[winuser::VK_CAPITAL as usize] &= !0x80;
        }
    }
}

pub struct Layout {
    /// Maps a modifier state to group of key strings
    /// Not using `ModifiersState` here because that object cannot express caps lock
    /// but we need to handle caps lock too.
    ///
    /// This map shouldn't need to exist.
    /// However currently this seems to be the only good way
    /// of getting the label for the pressed key. Note that calling `ToUnicode`
    /// just when the key is pressed/released would be enough if `ToUnicode` wouldn't
    /// change the keyboard state (it clears the dead key). There is a flag to prevent
    /// changing the state but that flag requires Windows 10, version 1607 or newer)
    pub keys: HashMap<WindowsModifiers, HashMap<KeyCode, Key<'static>>>,
    pub has_alt_graph: bool,
}

impl Layout {
    pub fn get_key(&self, mods: WindowsModifiers, scancode: ExScancode, keycode: KeyCode) -> Key<'static> {
        if let Some(keys) = self.keys.get(&mods) {
            if let Some(key) = keys.get(&keycode) {
                return *key;
            }
        }
        Key::Unidentified(NativeKeyCode::Windows(scancode))
    }
}

#[derive(Default)]
pub struct LayoutCache {
    /// Maps locale identifiers (HKL) to layouts
    pub layouts: HashMap<u64, Layout>,
    pub strings: HashSet<&'static str>,
}

impl LayoutCache {
    const SHIFT_FLAG: u8 = 1 << 0;
    const CONTROL_FLAG: u8 = 1 << 1;
    const ALT_FLAG: u8 = 1 << 2;
    const CAPS_LOCK_FLAG: u8 = 1 << 3;
    const MOD_FLAGS_END: u8 = 1 << 4;

    /// Checks whether the current layout is already known and
    /// prepares the layout if it isn't known.
    /// The current layout is then returned.
    pub fn get_current_layout(&mut self) -> (u64, &Layout) {
        let locale_id = unsafe { winuser::GetKeyboardLayout(0) } as u64;
        match self.layouts.entry(locale_id) {
            Entry::Occupied(entry) => {
                (locale_id, entry.get())
            }
            Entry::Vacant(entry) => {
                let layout = self.prepare_layout(locale_id);
                (locale_id, entry.insert(layout))
            }
        }
    }

    /// Returns Some if succeeded
    fn prepare_layout(&mut self, locale_identifier: u64) -> Layout {
        let mut layout = Layout {
            keys: Default::default(),
            has_alt_graph: false,
        };

        // We initialize the keyboard state with all zeros to
        // simulate a scenario when no modifier is active.
        let mut key_state = [0u8; 256];

        // Iterate through every combination of modifiers
        let mods_end = WindowsModifiers::FLAGS_END.bits;
        for mod_state in 0..mods_end {
            let mut keys_for_this_mod = HashMap::with_capacity(256);

            let mod_state = unsafe { WindowsModifiers::from_bits_unchecked(mod_state) };

            //Self::apply_mod_state(&mut key_state, mod_state);

            // Virtual key values are in the domain [0, 255].
            // This is reinforced by the fact that the keyboard state array has 256
            // elements. This array is allowed to be indexed by virtual key values
            // giving the key state for the virtual key used for indexing.
            for vk in 0..256 {
                let scancode = unsafe {
                    winuser::MapVirtualKeyExW(vk, winuser::MAPVK_VK_TO_VSC_EX, locale_identifier as HKL)
                };
                if scancode == 0 {
                    continue;
                }

                let native_code = NativeKeyCode::Windows(scancode as ExScancode);
                let key_code = native_key_to_code(scancode as ExScancode);
                // Let's try to get the key from just the scancode and vk
                // We don't necessarily know yet if AltGraph is present on this layout so we'll
                // assume it isn't. Then we'll do a second pass where we set the "AltRight" keys to
                // "AltGr" in case we find out that there's an AltGraph.
                let preliminary_key = vkey_to_non_printable(
                    vk as i32, native_code, key_code, locale_identifier, false
                );
                match preliminary_key {
                    Key::Unidentified(_) => (),
                    _ => {
                        keys_for_this_mod.insert(key_code, preliminary_key);
                        continue;
                    }
                }

                let unicode = Self::to_unicode_string(&key_state, vk, scancode, locale_identifier);
                let key = match unicode {
                    ToUnicodeResult::Str(str) => {
                        let static_str = self.get_or_insert_str(str);
                        Key::Character(static_str)
                    }
                    ToUnicodeResult::Dead(dead_char) => {
                        Key::Dead(dead_char)
                    }
                    ToUnicodeResult::None => {
                        // Just use the unidentified key, we got earlier
                        preliminary_key
                    }
                };

                // Check for alt graph.
                // The logic is that if a key pressed with the CTRL modifier produces
                // a different result from when it's pressed with CTRL+ALT then the layout
                // has AltGr.
                const CTRL_ALT: WindowsModifiers =
                    WindowsModifiers::CONTROL | WindowsModifiers::ALT;
                let is_in_ctrl_alt = (mod_state & CTRL_ALT) == CTRL_ALT;
                if !layout.has_alt_graph && is_in_ctrl_alt {
                    // Unwrapping here because if we are in the ctrl+alt modifier state
                    // then the alt modifier state must have come before.
                    let alt_keys = layout.keys.get(&WindowsModifiers::ALT).unwrap();
                    if let Some(key_without_ctrl_alt) = alt_keys.get(&key_code) {
                        layout.has_alt_graph = key != *key_without_ctrl_alt;
                    }
                }

                keys_for_this_mod.insert(key_code, key);
            }

            layout.keys.insert(mod_state, keys_for_this_mod);
        }

        // Second pass: replace right alt keys with AltGr if the layout has alt graph
        if layout.has_alt_graph {
            for mod_state in 0..mods_end {
                let mod_state = unsafe { WindowsModifiers::from_bits_unchecked(mod_state) };
                if let Some(keys) = layout.keys.get_mut(&mod_state) {
                    if let Some(key) = keys.get_mut(&KeyCode::AltRight) {
                        *key = Key::AltGraph;
                    }
                }
            }
        }

        layout
    }

    pub fn get_or_insert_str(&mut self, string: String) -> &'static str {
        {
            let str_ref = string.as_str();
            if let Some(&existing) = self.strings.get(&str_ref) {
                return existing;
            }
        }
        let leaked = Box::leak(Box::from(string));
        self.strings.insert(leaked);
        leaked
    }

    fn to_unicode_string(
        key_state: &[u8; 256],
        vkey: u32,
        scancode: u32,
        locale_identifier: u64,
    ) -> ToUnicodeResult {
        unsafe {
            let mut label_wide = [0u16; 8];
            let mut wide_len = winuser::ToUnicodeEx(
                vkey,
                scancode,
                (&key_state[0]) as *const _,
                (&mut label_wide[0]) as *mut _,
                label_wide.len() as i32,
                0,
                locale_identifier as _,
            );
            if wide_len < 0 {
                // If it's dead, let's run `ToUnicode` again, to consume the dead-key
                wide_len = winuser::ToUnicodeEx(
                    vkey,
                    scancode,
                    (&key_state[0]) as *const _,
                    (&mut label_wide[0]) as *mut _,
                    label_wide.len() as i32,
                    0,
                    locale_identifier as _,
                );
                if wide_len > 0 {
                    let os_string = OsString::from_wide(&label_wide[0..wide_len as usize]);
                    if let Ok(label_str) = os_string.into_string() {
                        if let Some(ch) = label_str.chars().next() {
                            return ToUnicodeResult::Dead(Some(ch));
                        }
                    }
                }
                return ToUnicodeResult::Dead(None);
            }
            if wide_len > 0 {
                let os_string = OsString::from_wide(&label_wide[0..wide_len as usize]);
                if let Ok(label_str) = os_string.into_string() {
                    return ToUnicodeResult::Str(label_str);
                }
            }
        }
        ToUnicodeResult::None
    }
}

#[derive(Clone, Eq, PartialEq)]
enum ToUnicodeResult {
    Str(String),
    Dead(Option<char>),
    None,
}

impl ToUnicodeResult {
    fn is_none(&self) -> bool {
        match self {
            ToUnicodeResult::None => true,
            _ => false,
        }
    }
}
