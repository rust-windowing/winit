use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    sync::Mutex,
};

use lazy_static::lazy_static;

use winapi::{
    ctypes::c_int,
    shared::minwindef::{HKL, LOWORD},
    um::{
        winnt::{LANG_JAPANESE, LANG_KOREAN, PRIMARYLANGID},
        winuser,
    },
};

use crate::{
    keyboard::{Key, KeyCode, ModifiersState, NativeKeyCode},
    platform::scancode::KeyCodeExtScancode,
    platform_impl::platform::keyboard::ExScancode,
};

lazy_static! {
    pub(crate) static ref LAYOUT_CACHE: Mutex<LayoutCache> = Mutex::new(LayoutCache::default());
}

fn key_pressed(vkey: c_int) -> bool {
    unsafe { (winuser::GetKeyState(vkey) & (1 << 15)) == (1 << 15) }
}

const NUMPAD_VKEYS: [c_int; 16] = [
    winuser::VK_NUMPAD0,
    winuser::VK_NUMPAD1,
    winuser::VK_NUMPAD2,
    winuser::VK_NUMPAD3,
    winuser::VK_NUMPAD4,
    winuser::VK_NUMPAD5,
    winuser::VK_NUMPAD6,
    winuser::VK_NUMPAD7,
    winuser::VK_NUMPAD8,
    winuser::VK_NUMPAD9,
    winuser::VK_MULTIPLY,
    winuser::VK_ADD,
    winuser::VK_SEPARATOR,
    winuser::VK_SUBTRACT,
    winuser::VK_DECIMAL,
    winuser::VK_DIVIDE,
];

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

    pub fn apply_to_kbd_state(self, key_state: &mut [u8; 256]) {
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
            key_state[winuser::VK_CAPITAL as usize] |= 0x01;
        } else {
            key_state[winuser::VK_CAPITAL as usize] &= !0x01;
        }
    }

    /// Removes the control modifier if the alt modifier is not present.
    /// This is useful because on Windows: (Control + Alt) == AltGr
    /// but we don't want to interfere with the AltGr state.
    pub fn remove_only_ctrl(mut self) -> WindowsModifiers {
        if !self.contains(WindowsModifiers::ALT) {
            self.remove(WindowsModifiers::CONTROL);
        }
        self
    }
}

pub(crate) struct Layout {
    pub hkl: u64,

    /// Maps a Windows virtual key to a `Key`.
    ///
    /// The only keys that are mapped are ones which don't require knowing the modifier state when
    /// mapping. Making this field separate from the `keys` field saves having to add NumLock as a
    /// modifier to `WindowsModifiers`, which would double the number of items in keys.
    pub simple_vkeys: HashMap<c_int, Key<'static>>,

    /// Maps a modifier state to group of key strings
    /// We're not using `ModifiersState` here because that object cannot express caps lock,
    /// but we need to handle caps lock too.
    ///
    /// This map shouldn't need to exist.
    /// However currently this seems to be the only good way
    /// of getting the label for the pressed key. Note that calling `ToUnicode`
    /// just when the key is pressed/released would be enough if `ToUnicode` wouldn't
    /// change the keyboard state (it clears the dead key). There is a flag to prevent
    /// changing the state, but that flag requires Windows 10, version 1607 or newer)
    pub keys: HashMap<WindowsModifiers, HashMap<KeyCode, Key<'static>>>,
    pub has_alt_graph: bool,
}

impl Layout {
    pub fn get_key(
        &self,
        mods: WindowsModifiers,
        vkey: c_int,
        scancode: ExScancode,
        keycode: KeyCode,
    ) -> Key<'static> {
        let native_code = NativeKeyCode::Windows(scancode);

        let unknown_alt = vkey == winuser::VK_MENU;
        if !unknown_alt {
            // Here we try using the virtual key directly but if the virtual key doesn't distinguish
            // between left and right alt, we can't report AltGr. Therefore, we only do this if the
            // key is not the "unknown alt" key.
            //
            // The reason for using the virtual key directly is that `MapVirtualKeyExW` (used when
            // building the keys map) sometimes maps virtual keys to odd scancodes that don't match
            // the scancode coming from the KEYDOWN message for the same key. For example: `VK_LEFT`
            // is mapped to `0x004B`, but the scancode for the left arrow is `0xE04B`.
            let key_from_vkey =
                vkey_to_non_char_key(vkey, native_code, self.hkl, self.has_alt_graph);

            if !matches!(key_from_vkey, Key::Unidentified(_)) {
                return key_from_vkey;
            }
        }
        if let Some(key) = self.simple_vkeys.get(&vkey) {
            return *key;
        }
        if let Some(keys) = self.keys.get(&mods) {
            if let Some(key) = keys.get(&keycode) {
                return *key;
            }
        }
        Key::Unidentified(native_code)
    }
}

#[derive(Default)]
pub(crate) struct LayoutCache {
    /// Maps locale identifiers (HKL) to layouts
    pub layouts: HashMap<u64, Layout>,
    pub strings: HashSet<&'static str>,
}

impl LayoutCache {
    /// Checks whether the current layout is already known and
    /// prepares the layout if it isn't known.
    /// The current layout is then returned.
    pub fn get_current_layout<'a>(&'a mut self) -> (u64, &'a Layout) {
        let locale_id = unsafe { winuser::GetKeyboardLayout(0) } as u64;
        match self.layouts.entry(locale_id) {
            Entry::Occupied(entry) => (locale_id, entry.into_mut()),
            Entry::Vacant(entry) => {
                let layout = Self::prepare_layout(&mut self.strings, locale_id);
                (locale_id, entry.insert(layout))
            }
        }
    }

    pub fn get_agnostic_mods(&mut self) -> ModifiersState {
        let (_, layout) = self.get_current_layout();
        let filter_out_altgr = layout.has_alt_graph && key_pressed(winuser::VK_RMENU);
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
            ModifiersState::SUPER,
            key_pressed(winuser::VK_LWIN) || key_pressed(winuser::VK_RWIN),
        );
        mods
    }

    fn prepare_layout(strings: &mut HashSet<&'static str>, locale_id: u64) -> Layout {
        let mut layout = Layout {
            hkl: locale_id,
            simple_vkeys: Default::default(),
            keys: Default::default(),
            has_alt_graph: false,
        };

        // We initialize the keyboard state with all zeros to
        // simulate a scenario when no modifier is active.
        let mut key_state = [0u8; 256];

        // First, generate all the simple vkeys.
        // Some numpad keys generate different charcaters based on the locale.
        // For example `VK_DECIMAL` is sometimes "." and sometimes ","
        layout.simple_vkeys.reserve(NUMPAD_VKEYS.len());
        for vk in NUMPAD_VKEYS.iter() {
            let vk = (*vk) as u32;
            let scancode = unsafe {
                winuser::MapVirtualKeyExW(vk, winuser::MAPVK_VK_TO_VSC_EX, locale_id as HKL)
            };
            let unicode = Self::to_unicode_string(&key_state, vk, scancode, locale_id);
            if let ToUnicodeResult::Str(s) = unicode {
                let static_str = get_or_insert_str(strings, s);
                layout
                    .simple_vkeys
                    .insert(vk as i32, Key::Character(static_str));
            }
        }

        // Iterate through every combination of modifiers
        let mods_end = WindowsModifiers::FLAGS_END.bits;
        for mod_state in 0..mods_end {
            let mut keys_for_this_mod = HashMap::with_capacity(256);

            let mod_state = unsafe { WindowsModifiers::from_bits_unchecked(mod_state) };
            mod_state.apply_to_kbd_state(&mut key_state);

            // Virtual key values are in the domain [0, 255].
            // This is reinforced by the fact that the keyboard state array has 256
            // elements. This array is allowed to be indexed by virtual key values
            // giving the key state for the virtual key used for indexing.
            for vk in 0..256 {
                let scancode = unsafe {
                    winuser::MapVirtualKeyExW(vk, winuser::MAPVK_VK_TO_VSC_EX, locale_id as HKL)
                };
                if scancode == 0 {
                    continue;
                }

                let native_code = NativeKeyCode::Windows(scancode as ExScancode);
                let key_code = KeyCode::from_scancode(scancode);
                // Let's try to get the key from just the scancode and vk
                // We don't necessarily know yet if AltGraph is present on this layout so we'll
                // assume it isn't. Then we'll do a second pass where we set the "AltRight" keys to
                // "AltGr" in case we find out that there's an AltGraph.
                let preliminary_key =
                    vkey_to_non_char_key(vk as i32, native_code, locale_id, false);
                match preliminary_key {
                    Key::Unidentified(_) => (),
                    _ => {
                        keys_for_this_mod.insert(key_code, preliminary_key);
                        continue;
                    }
                }

                let unicode = Self::to_unicode_string(&key_state, vk, scancode, locale_id);
                let key = match unicode {
                    ToUnicodeResult::Str(str) => {
                        let static_str = get_or_insert_str(strings, str);
                        Key::Character(static_str)
                    }
                    ToUnicodeResult::Dead(dead_char) => {
                        //println!("{:?} - {:?} produced dead {:?}", key_code, mod_state, dead_char);
                        Key::Dead(dead_char)
                    }
                    ToUnicodeResult::None => {
                        let has_alt = mod_state.contains(WindowsModifiers::ALT);
                        let has_ctrl = mod_state.contains(WindowsModifiers::CONTROL);
                        // HACK: `ToUnicodeEx` seems to fail getting the string for the numpad
                        // divide key, so we handle that explicitly here
                        if !has_alt && !has_ctrl && key_code == KeyCode::NumpadDivide {
                            Key::Character("/")
                        } else {
                            // Just use the unidentified key, we got earlier
                            preliminary_key
                        }
                    }
                };

                // Check for alt graph.
                // The logic is that if a key pressed with no modifier produces
                // a different `Character` from when it's pressed with CTRL+ALT then the layout
                // has AltGr.
                let ctrl_alt: WindowsModifiers = WindowsModifiers::CONTROL | WindowsModifiers::ALT;
                let is_in_ctrl_alt = mod_state == ctrl_alt;
                if !layout.has_alt_graph && is_in_ctrl_alt {
                    // Unwrapping here because if we are in the ctrl+alt modifier state
                    // then the alt modifier state must have come before.
                    let simple_keys = layout.keys.get(&WindowsModifiers::empty()).unwrap();
                    if let Some(Key::Character(key_no_altgr)) = simple_keys.get(&key_code) {
                        if let Key::Character(key) = key {
                            layout.has_alt_graph = key != *key_no_altgr;
                        }
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

    fn to_unicode_string(
        key_state: &[u8; 256],
        vkey: u32,
        scancode: u32,
        locale_id: u64,
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
                locale_id as HKL,
            );
            if wide_len < 0 {
                // If it's dead, we run `ToUnicode` again to consume the dead-key
                wide_len = winuser::ToUnicodeEx(
                    vkey,
                    scancode,
                    (&key_state[0]) as *const _,
                    (&mut label_wide[0]) as *mut _,
                    label_wide.len() as i32,
                    0,
                    locale_id as HKL,
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

pub fn get_or_insert_str<T>(strings: &mut HashSet<&'static str>, string: T) -> &'static str
where
    T: AsRef<str>,
    String: From<T>,
{
    {
        let str_ref = string.as_ref();
        if let Some(&existing) = strings.get(str_ref) {
            return existing;
        }
    }
    let leaked = Box::leak(Box::from(String::from(string)));
    strings.insert(leaked);
    leaked
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ToUnicodeResult {
    Str(String),
    Dead(Option<char>),
    None,
}

/// This converts virtual keys to `Key`s. Only virtual keys which can be unambiguously converted to
/// a `Key`, with only the information passed in as arguments, are converted.
///
/// In other words: this function does not need to "prepare" the current layout in order to do
/// the conversion, but as such it cannot convert certain keys, like language-specific character keys.
///
/// The result includes all non-character keys defined within `Key` plus characters from numpad keys.
/// For example, backspace and tab are included.
fn vkey_to_non_char_key(
    vkey: i32,
    native_code: NativeKeyCode,
    hkl: u64,
    has_alt_graph: bool,
) -> Key<'static> {
    // List of the Web key names and their corresponding platform-native key names:
    // https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/key/Key_Values

    let primary_lang_id = PRIMARYLANGID(LOWORD(hkl as u32));
    let is_korean = primary_lang_id == LANG_KOREAN;
    let is_japanese = primary_lang_id == LANG_JAPANESE;

    match vkey {
        winuser::VK_LBUTTON => Key::Unidentified(NativeKeyCode::Unidentified), // Mouse
        winuser::VK_RBUTTON => Key::Unidentified(NativeKeyCode::Unidentified), // Mouse

        // I don't think this can be represented with a Key
        winuser::VK_CANCEL => Key::Unidentified(native_code),

        winuser::VK_MBUTTON => Key::Unidentified(NativeKeyCode::Unidentified), // Mouse
        winuser::VK_XBUTTON1 => Key::Unidentified(NativeKeyCode::Unidentified), // Mouse
        winuser::VK_XBUTTON2 => Key::Unidentified(NativeKeyCode::Unidentified), // Mouse
        winuser::VK_BACK => Key::Backspace,
        winuser::VK_TAB => Key::Tab,
        winuser::VK_CLEAR => Key::Clear,
        winuser::VK_RETURN => Key::Enter,
        winuser::VK_SHIFT => Key::Shift,
        winuser::VK_CONTROL => Key::Control,
        winuser::VK_MENU => Key::Alt,
        winuser::VK_PAUSE => Key::Pause,
        winuser::VK_CAPITAL => Key::CapsLock,

        //winuser::VK_HANGEUL => Key::HangulMode, // Deprecated in favour of VK_HANGUL

        // VK_HANGUL and VK_KANA are defined as the same constant, therefore
        // we use appropriate conditions to differentate between them
        winuser::VK_HANGUL if is_korean => Key::HangulMode,
        winuser::VK_KANA if is_japanese => Key::KanaMode,

        winuser::VK_JUNJA => Key::JunjaMode,
        winuser::VK_FINAL => Key::FinalMode,

        // VK_HANJA and VK_KANJI are defined as the same constant, therefore
        // we use appropriate conditions to differentate between them
        winuser::VK_HANJA if is_korean => Key::HanjaMode,
        winuser::VK_KANJI if is_japanese => Key::KanjiMode,

        winuser::VK_ESCAPE => Key::Escape,
        winuser::VK_CONVERT => Key::Convert,
        winuser::VK_NONCONVERT => Key::NonConvert,
        winuser::VK_ACCEPT => Key::Accept,
        winuser::VK_MODECHANGE => Key::ModeChange,
        winuser::VK_SPACE => Key::Space,
        winuser::VK_PRIOR => Key::PageUp,
        winuser::VK_NEXT => Key::PageDown,
        winuser::VK_END => Key::End,
        winuser::VK_HOME => Key::Home,
        winuser::VK_LEFT => Key::ArrowLeft,
        winuser::VK_UP => Key::ArrowUp,
        winuser::VK_RIGHT => Key::ArrowRight,
        winuser::VK_DOWN => Key::ArrowDown,
        winuser::VK_SELECT => Key::Select,
        winuser::VK_PRINT => Key::Print,
        winuser::VK_EXECUTE => Key::Execute,
        winuser::VK_SNAPSHOT => Key::PrintScreen,
        winuser::VK_INSERT => Key::Insert,
        winuser::VK_DELETE => Key::Delete,
        winuser::VK_HELP => Key::Help,
        winuser::VK_LWIN => Key::Super,
        winuser::VK_RWIN => Key::Super,
        winuser::VK_APPS => Key::ContextMenu,
        winuser::VK_SLEEP => Key::Standby,

        // Numpad keys produce characters
        winuser::VK_NUMPAD0 => Key::Unidentified(native_code),
        winuser::VK_NUMPAD1 => Key::Unidentified(native_code),
        winuser::VK_NUMPAD2 => Key::Unidentified(native_code),
        winuser::VK_NUMPAD3 => Key::Unidentified(native_code),
        winuser::VK_NUMPAD4 => Key::Unidentified(native_code),
        winuser::VK_NUMPAD5 => Key::Unidentified(native_code),
        winuser::VK_NUMPAD6 => Key::Unidentified(native_code),
        winuser::VK_NUMPAD7 => Key::Unidentified(native_code),
        winuser::VK_NUMPAD8 => Key::Unidentified(native_code),
        winuser::VK_NUMPAD9 => Key::Unidentified(native_code),
        winuser::VK_MULTIPLY => Key::Unidentified(native_code),
        winuser::VK_ADD => Key::Unidentified(native_code),
        winuser::VK_SEPARATOR => Key::Unidentified(native_code),
        winuser::VK_SUBTRACT => Key::Unidentified(native_code),
        winuser::VK_DECIMAL => Key::Unidentified(native_code),
        winuser::VK_DIVIDE => Key::Unidentified(native_code),

        winuser::VK_F1 => Key::F1,
        winuser::VK_F2 => Key::F2,
        winuser::VK_F3 => Key::F3,
        winuser::VK_F4 => Key::F4,
        winuser::VK_F5 => Key::F5,
        winuser::VK_F6 => Key::F6,
        winuser::VK_F7 => Key::F7,
        winuser::VK_F8 => Key::F8,
        winuser::VK_F9 => Key::F9,
        winuser::VK_F10 => Key::F10,
        winuser::VK_F11 => Key::F11,
        winuser::VK_F12 => Key::F12,
        winuser::VK_F13 => Key::F13,
        winuser::VK_F14 => Key::F14,
        winuser::VK_F15 => Key::F15,
        winuser::VK_F16 => Key::F16,
        winuser::VK_F17 => Key::F17,
        winuser::VK_F18 => Key::F18,
        winuser::VK_F19 => Key::F19,
        winuser::VK_F20 => Key::F20,
        winuser::VK_F21 => Key::F21,
        winuser::VK_F22 => Key::F22,
        winuser::VK_F23 => Key::F23,
        winuser::VK_F24 => Key::F24,
        winuser::VK_NAVIGATION_VIEW => Key::Unidentified(native_code),
        winuser::VK_NAVIGATION_MENU => Key::Unidentified(native_code),
        winuser::VK_NAVIGATION_UP => Key::Unidentified(native_code),
        winuser::VK_NAVIGATION_DOWN => Key::Unidentified(native_code),
        winuser::VK_NAVIGATION_LEFT => Key::Unidentified(native_code),
        winuser::VK_NAVIGATION_RIGHT => Key::Unidentified(native_code),
        winuser::VK_NAVIGATION_ACCEPT => Key::Unidentified(native_code),
        winuser::VK_NAVIGATION_CANCEL => Key::Unidentified(native_code),
        winuser::VK_NUMLOCK => Key::NumLock,
        winuser::VK_SCROLL => Key::ScrollLock,
        winuser::VK_OEM_NEC_EQUAL => Key::Unidentified(native_code),
        //winuser::VK_OEM_FJ_JISHO => Key::Unidentified(native_code), // Conflicts with `VK_OEM_NEC_EQUAL`
        winuser::VK_OEM_FJ_MASSHOU => Key::Unidentified(native_code),
        winuser::VK_OEM_FJ_TOUROKU => Key::Unidentified(native_code),
        winuser::VK_OEM_FJ_LOYA => Key::Unidentified(native_code),
        winuser::VK_OEM_FJ_ROYA => Key::Unidentified(native_code),
        winuser::VK_LSHIFT => Key::Shift,
        winuser::VK_RSHIFT => Key::Shift,
        winuser::VK_LCONTROL => Key::Control,
        winuser::VK_RCONTROL => Key::Control,
        winuser::VK_LMENU => Key::Alt,
        winuser::VK_RMENU => {
            if has_alt_graph {
                Key::AltGraph
            } else {
                Key::Alt
            }
        }
        winuser::VK_BROWSER_BACK => Key::BrowserBack,
        winuser::VK_BROWSER_FORWARD => Key::BrowserForward,
        winuser::VK_BROWSER_REFRESH => Key::BrowserRefresh,
        winuser::VK_BROWSER_STOP => Key::BrowserStop,
        winuser::VK_BROWSER_SEARCH => Key::BrowserSearch,
        winuser::VK_BROWSER_FAVORITES => Key::BrowserFavorites,
        winuser::VK_BROWSER_HOME => Key::BrowserHome,
        winuser::VK_VOLUME_MUTE => Key::AudioVolumeMute,
        winuser::VK_VOLUME_DOWN => Key::AudioVolumeDown,
        winuser::VK_VOLUME_UP => Key::AudioVolumeUp,
        winuser::VK_MEDIA_NEXT_TRACK => Key::MediaTrackNext,
        winuser::VK_MEDIA_PREV_TRACK => Key::MediaTrackPrevious,
        winuser::VK_MEDIA_STOP => Key::MediaStop,
        winuser::VK_MEDIA_PLAY_PAUSE => Key::MediaPlayPause,
        winuser::VK_LAUNCH_MAIL => Key::LaunchMail,
        winuser::VK_LAUNCH_MEDIA_SELECT => Key::LaunchMediaPlayer,
        winuser::VK_LAUNCH_APP1 => Key::LaunchApplication1,
        winuser::VK_LAUNCH_APP2 => Key::LaunchApplication2,

        // This function only converts "non-printable"
        winuser::VK_OEM_1 => Key::Unidentified(native_code),
        winuser::VK_OEM_PLUS => Key::Unidentified(native_code),
        winuser::VK_OEM_COMMA => Key::Unidentified(native_code),
        winuser::VK_OEM_MINUS => Key::Unidentified(native_code),
        winuser::VK_OEM_PERIOD => Key::Unidentified(native_code),
        winuser::VK_OEM_2 => Key::Unidentified(native_code),
        winuser::VK_OEM_3 => Key::Unidentified(native_code),

        winuser::VK_GAMEPAD_A => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_B => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_X => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_Y => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_RIGHT_SHOULDER => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_LEFT_SHOULDER => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_LEFT_TRIGGER => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_RIGHT_TRIGGER => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_DPAD_UP => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_DPAD_DOWN => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_DPAD_LEFT => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_DPAD_RIGHT => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_MENU => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_VIEW => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_LEFT_THUMBSTICK_UP => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_LEFT_THUMBSTICK_DOWN => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_LEFT_THUMBSTICK_RIGHT => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_LEFT_THUMBSTICK_LEFT => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_UP => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_DOWN => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_RIGHT => Key::Unidentified(native_code),
        winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT => Key::Unidentified(native_code),

        // This function only converts "non-printable"
        winuser::VK_OEM_4 => Key::Unidentified(native_code),
        winuser::VK_OEM_5 => Key::Unidentified(native_code),
        winuser::VK_OEM_6 => Key::Unidentified(native_code),
        winuser::VK_OEM_7 => Key::Unidentified(native_code),
        winuser::VK_OEM_8 => Key::Unidentified(native_code),
        winuser::VK_OEM_AX => Key::Unidentified(native_code),
        winuser::VK_OEM_102 => Key::Unidentified(native_code),

        winuser::VK_ICO_HELP => Key::Unidentified(native_code),
        winuser::VK_ICO_00 => Key::Unidentified(native_code),

        winuser::VK_PROCESSKEY => Key::Process,

        winuser::VK_ICO_CLEAR => Key::Unidentified(native_code),
        winuser::VK_PACKET => Key::Unidentified(native_code),
        winuser::VK_OEM_RESET => Key::Unidentified(native_code),
        winuser::VK_OEM_JUMP => Key::Unidentified(native_code),
        winuser::VK_OEM_PA1 => Key::Unidentified(native_code),
        winuser::VK_OEM_PA2 => Key::Unidentified(native_code),
        winuser::VK_OEM_PA3 => Key::Unidentified(native_code),
        winuser::VK_OEM_WSCTRL => Key::Unidentified(native_code),
        winuser::VK_OEM_CUSEL => Key::Unidentified(native_code),

        winuser::VK_OEM_ATTN => Key::Attn,
        winuser::VK_OEM_FINISH => {
            if is_japanese {
                Key::Katakana
            } else {
                // This matches IE and Firefox behaviour according to
                // https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/key/Key_Values
                // At the time of writing, there is no `Key::Finish` variant as
                // Finish is not mentionned at https://w3c.github.io/uievents-key/
                // Also see: https://github.com/pyfisch/keyboard-types/issues/9
                Key::Unidentified(native_code)
            }
        }
        winuser::VK_OEM_COPY => Key::Copy,
        winuser::VK_OEM_AUTO => Key::Hankaku,
        winuser::VK_OEM_ENLW => Key::Zenkaku,
        winuser::VK_OEM_BACKTAB => Key::Romaji,
        winuser::VK_ATTN => Key::KanaMode,
        winuser::VK_CRSEL => Key::CrSel,
        winuser::VK_EXSEL => Key::ExSel,
        winuser::VK_EREOF => Key::EraseEof,
        winuser::VK_PLAY => Key::Play,
        winuser::VK_ZOOM => Key::ZoomToggle,
        winuser::VK_NONAME => Key::Unidentified(native_code),
        winuser::VK_PA1 => Key::Unidentified(native_code),
        winuser::VK_OEM_CLEAR => Key::Clear,
        _ => Key::Unidentified(native_code),
    }
}
