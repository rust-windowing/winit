use std::{
    os::raw::c_int, fmt, collections::HashMap,
    char, mem::MaybeUninit, ffi::OsString, os::windows::ffi::OsStringExt
};

use keyboard_types::Key;

use winapi::{
    shared::{minwindef::{LRESULT, LPARAM, WPARAM, HKL, LOWORD}, windef::HWND},
    um::{winuser, winnt::{PRIMARYLANGID, LANG_KOREAN, LANG_JAPANESE}}
};


use crate::{
    event::{KeyEvent, ScanCode},
    platform_impl::platform::event::KeyEventExtra,
};

pub fn is_msg_keyboard_related(msg: u32) -> bool {
    use winuser::{WM_KEYFIRST, WM_KEYLAST, WM_SETFOCUS, WM_KILLFOCUS, WM_INPUTLANGCHANGE, WM_SHOWWINDOW};
    let is_keyboard_msg = WM_KEYFIRST <= msg && msg <= WM_KEYLAST;

    is_keyboard_msg
}

#[derive(Debug, Copy, Clone)]
pub struct KeyLParam {
    pub scancode: u8,
    pub extended: bool,

    /// This is `previous_state XOR transition_state` see the lParam for WM_KEYDOWN and WM_KEYUP.
    pub is_repeat: bool,
}

#[derive(Eq, PartialEq)]
enum ToUnicodeResult {
    Str(String),
    Dead,
    None,
}

impl ToUnicodeResult {
    fn is_none(&self) -> bool {
        match self {
            ToUnicodeResult::None => true,
            _ => false
        }
    }

    fn is_something(&self) -> bool {
        !self.is_none()
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct PlatformScanCode(pub u16);
impl fmt::Debug for PlatformScanCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PlatformScanCode").field(&format_args!("0x{:04x}", self.0)).finish()
    }
}
impl PlatformScanCode {
    pub fn new(scancode: u8, extended: bool) -> PlatformScanCode {
        let ex_scancode = (scancode as u16) | (if extended { 0xE000 } else { 0 });
        PlatformScanCode(ex_scancode)
    }
}

/// Stores information required to make `KeyEvent`s.
/// 
/// A single winint `KeyEvent` contains information which the windows API passes to the application
/// in multiple window messages. In other words a winit `KeyEvent` cannot be build from a single
/// window message. Therefore this type keeps track of certain information from previous events so
/// that a `KeyEvent` can be constructed when the last event related to a keypress is received. 
///
/// `PeekMessage` is sometimes used to determine wheter the next window message still belongs to the
/// current keypress. If it doesn't and the current state represents a key event waiting to be
/// dispatched, than said event is considered complete and is dispatched.
///
/// The sequence of window messages for a key press event is the following:
/// - Exactly one WM_KEYDOWN / WM_SYSKEYDOWN
/// - Zero or one WM_DEADCHAR / WM_SYSDEADCHAR
/// - Zero or more WM_CHAR / WM_SYSCHAR. These messages each come with a UTF-16 code unit which when
/// put together in the sequence they arrived in, forms the text which is the result of pressing the
/// key.
/// 
/// Key release messages are a bit different due to the fact that they don't contribute to
/// text input. The "sequence" only consists of one WM_KEYUP / WM_SYSKEYUP event.
pub struct KeyEventBuilder {
    event_info: Option<PartialKeyEventInfo>,

    /// This map shouldn't need to exist.
    /// However currently this seems to be the only good way
    /// of getting the label for the pressed key. Note that calling `ToUnicode`
    /// just when the key is pressed/released would be enough if `ToUnicode` wouldn't
    /// change the keyboard state (it clears the dead key). There is a flag to prevent
    /// changing the state but that flag requires Windows 10, version 1607 or newer)
    key_labels: HashMap<PlatformScanCode, String>,

    /// True if the keyboard layout belonging to `known_locale_id` has an AltGr key.
    has_alt_graph: bool,

    /// The locale identifier (HKL) of the layout for which the `key_labels` and `has_alt_graph` was
    /// generated
    known_locale_id: usize,

    /// The keyup event needs to call `ToUnicode` to determine what's the text produced by the
    /// key with all modifiers except CTRL (the `logical_key`).
    /// 
    /// But `ToUnicode` without the non-modifying flag (see `key_labels`), resets the dead key
    /// state which would be incorrect during every keyup event. Therefore this variable is used
    /// to determine whether the last keydown event produced a dead key.
    ///
    /// Note that this variable is not always correct because it does
    /// not track key presses outside of this window. However the ONLY situation where this
    /// doesn't work as intended is when the user presses a dead key outside of this window, and
    /// switches to this window BEFORE releasing it then releases the dead key. In this case
    /// the `ToUnicode` function will be called, incorrectly clearing the dead key state. Having
    /// an inccorect behaviour only in this case seems acceptable.
    prev_down_was_dead: bool,
}
impl Default for KeyEventBuilder {
    fn default() -> Self {
        KeyEventBuilder {
            event_info: None,
            key_labels: HashMap::with_capacity(128),
            has_alt_graph: false,
            known_locale_id: 0,
            prev_down_was_dead: false,
        }
    }
}
impl KeyEventBuilder {
    const SHIFT_FLAG: u8 = 1 << 0;
    const CONTROL_FLAG: u8 = 1 << 1;
    const ALT_FLAG: u8 = 1 << 2;
    const CAPS_LOCK_FLAG: u8 = 1 << 3;

    /// Call this function for every window message.
    /// Returns Some() if this window message completes a KeyEvent.
    /// Returns None otherwise.
    pub fn process_message(
        &mut self,
        hwnd: HWND,
        msg_kind: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        retval: &mut LRESULT
    ) -> Option<KeyEvent> {
        match msg_kind {
            winuser::WM_KEYDOWN | winuser::WM_SYSKEYDOWN => {
                println!("{}, {}", file!(), line!());
                self.prev_down_was_dead = false;

                // When the labels are already generated for this locale,
                // the `generate_labels` function returns without calling
                // `ToUnicode` so it keeps the dead key state intact.
                let locale_id = unsafe { winuser::GetKeyboardLayout(0) };
                self.prepare_layout(locale_id as usize);

                let vkey = wparam as i32;
                let lparam_struct = destructure_key_lparam(lparam);
                let scancode = PlatformScanCode::new(
                    lparam_struct.scancode,
                    lparam_struct.extended
                );
                let code = native_key_to_code(scancode);
                let location = get_location(vkey, lparam_struct.extended);
                let label = self.key_labels.get(&scancode).map(|s| s.clone());
                self.event_info = Some(PartialKeyEventInfo {
                    key_state: keyboard_types::KeyState::Down,
                    vkey: wparam as i32,
                    scancode: scancode,
                    is_repeat: lparam_struct.is_repeat,
                    code: code,
                    location: location,
                    is_dead: false,
                    label: label,
                    utf16parts: Vec::with_capacity(8),
                    utf16parts_without_ctrl: Vec::with_capacity(8),
                });
                *retval = 0;
            }
            winuser::WM_DEADCHAR | winuser::WM_SYSDEADCHAR => {
                self.prev_down_was_dead = true;
                // At this point we know that there isn't going to be any more events related to
                // this key press
                let mut event_info = self.event_info.take().unwrap();
                event_info.is_dead = true;
                *retval = 0;
                return Some(event_info.finalize(self.known_locale_id, self.has_alt_graph));
            }
            winuser::WM_CHAR | winuser::WM_SYSCHAR => {
                println!("{}, {}", file!(), line!());
                *retval = 0;
                let is_high_surrogate = 0xD800 <= wparam && wparam <= 0xDBFF;
                let is_low_surrogate = 0xDC00 <= wparam && wparam <= 0xDFFF;

                let is_utf16 = is_high_surrogate || is_low_surrogate;

                let more_char_coming;
                unsafe {
                    let mut next_msg = MaybeUninit::uninit();
                    let has_message = winuser::PeekMessageW(
                        next_msg.as_mut_ptr(),
                        hwnd,
                        winuser::WM_KEYFIRST,
                        winuser::WM_KEYLAST,
                        winuser::PM_NOREMOVE
                    );
                    let has_message = has_message != 0;
                    if !has_message {
                        more_char_coming = false;
                    } else {
                        let next_msg = next_msg.assume_init().message;
                        if next_msg == winuser::WM_CHAR || next_msg == winuser::WM_SYSCHAR {
                            more_char_coming = true;
                        } else {
                            more_char_coming = false;
                        }
                    }
                }
                
                if is_utf16 {
                    self.event_info.as_mut().unwrap().utf16parts.push(wparam as u16);
                } else {
                    let utf16parts = &mut self.event_info.as_mut().unwrap().utf16parts;
                    let start_offset = utf16parts.len();
                    let new_size = utf16parts.len() + 2;
                    utf16parts.resize(new_size, 0);
                    if let Some(ch) = char::from_u32(wparam as u32) {
                        let encode_len =
                            ch.encode_utf16(&mut utf16parts[start_offset..]).len();
                        let new_size = start_offset + encode_len;
                        utf16parts.resize(new_size, 0);
                    }
                }
                if !more_char_coming {
                    let mut event_info = self.event_info.take().unwrap();

                    // Here it's okay to call `ToUnicode` because at this point the dead key
                    // is already consumed by the character.
                    unsafe {
                        let mut key_state: [MaybeUninit<u8>; 256] = [MaybeUninit::uninit(); 256];
                        winuser::GetKeyboardState(key_state[0].as_mut_ptr());
                        let mut key_state = std::mem::transmute::<_, [u8; 256]>(key_state);

                        let has_ctrl =
                            key_state[winuser::VK_CONTROL as usize] != 0 ||
                            key_state[winuser::VK_LCONTROL as usize] != 0 ||
                            key_state[winuser::VK_RCONTROL as usize] != 0;

                        // If neither of the CTRL keys is pressed, just use the text with all
                        // modifiers because that already consumed the dead key and otherwise
                        // we would interpret the character incorretly, missing the dead key.
                        if !has_ctrl {
                            event_info.utf16parts_without_ctrl = event_info.utf16parts.clone();
                        } else {
                            get_utf16_without_ctrl(
                                event_info.vkey as u32,
                                event_info.scancode,
                                &mut key_state,
                                &mut event_info.utf16parts_without_ctrl
                            );
                        }
                    }

                    return Some(event_info.finalize(self.known_locale_id, self.has_alt_graph));
                }
            }
            winuser::WM_KEYUP | winuser::WM_SYSKEYUP => {
                *retval = 0;
                let vkey = wparam as i32;
                let lparam_struct = destructure_key_lparam(lparam);
                let scancode = PlatformScanCode::new(
                    lparam_struct.scancode,
                    lparam_struct.extended
                );
                let code = native_key_to_code(scancode);
                let location = get_location(vkey, lparam_struct.extended);
                let mut utf16parts = Vec::with_capacity(8);
                let mut utf16parts_without_ctrl = Vec::with_capacity(8);

                // Avoid calling `ToUnicode` (which resets dead keys) if either the event
                // belongs to the key-down which just produced the dead key or if
                // the current key would not otherwise reset the dead key state.
                //
                // This logic relies on the assuption that keys which don't consume
                // dead keys, also do not produce text input.
                if !self.prev_down_was_dead && does_vkey_consume_dead_key(wparam as u32) {
                    unsafe {
                        //let locale_id = winuser::GetKeyboardLayout(0);
                        let mut key_state: [MaybeUninit<u8>; 256] = [MaybeUninit::uninit(); 256];
                        winuser::GetKeyboardState(key_state[0].as_mut_ptr());
                        let mut key_state = std::mem::transmute::<_, [u8; 256]>(key_state);
                        let unicode_len = winuser::ToUnicode(
                            wparam as u32,
                            scancode.0 as u32,
                            (&mut key_state[0]) as *mut _,
                            utf16parts.as_mut_ptr(),
                            utf16parts.capacity() as i32,
                            0
                        );
                        utf16parts.set_len(unicode_len as usize);
                        
                        get_utf16_without_ctrl(
                            wparam as u32,
                            scancode,
                            &mut key_state,
                            &mut utf16parts_without_ctrl
                        );
                    }
                }
                let label = self.key_labels.get(&scancode).map(|s| s.clone());
                let event_info = PartialKeyEventInfo {
                    key_state: keyboard_types::KeyState::Up,
                    vkey: wparam as i32,
                    scancode: scancode,
                    is_repeat: false,
                    code: code,
                    location: location,
                    is_dead: self.prev_down_was_dead,
                    label: label,
                    utf16parts: utf16parts,
                    utf16parts_without_ctrl: utf16parts_without_ctrl,
                };
                return Some(event_info.finalize(self.known_locale_id, self.has_alt_graph));
            }
            _ => ()
        }

        None
    }

    /// Returns true if succeeded.
    fn prepare_layout(&mut self, locale_identifier: usize) -> bool {
        if self.known_locale_id == locale_identifier {
            return true;
        }

        // We initialize the keyboard state with all zeros to
        // simulate a scenario when no modifier is active.
        let mut key_state = [0u8; 256];
        self.key_labels.clear();
        // Virtual key values are in the domain [0, 255].
        // This is reinforced by the fact that the keyboard state array has 256
        // elements. This array is allowed to be indexed by virtual key values
        // giving the key state for the virtual key used for indexing.
        for vk in 0..256 {
            let scancode = unsafe { winuser::MapVirtualKeyExW(
                vk,
                winuser::MAPVK_VK_TO_VSC_EX,
                locale_identifier as HKL
            ) };
            if scancode == 0 {
                continue;
            }
            Self::apply_mod_state(&mut key_state, 0);
            let unicode = Self::ToUnicodeString(&key_state, vk, scancode, locale_identifier);
            let unicode_str = match unicode {
                ToUnicodeResult::Str(str) => str,
                _ => continue,
            };
            let platform_scancode = PlatformScanCode(scancode as u16);
            self.key_labels.insert(platform_scancode, unicode_str);

            // Check for alt graph.
            // The logic is that if a key pressed with the CTRL modifier produces
            // a different result from when it's pressed with CTRL+ALT then the layout
            // has AltGr.
            if !self.has_alt_graph {
                Self::apply_mod_state(&mut key_state, Self::CONTROL_FLAG);
                let key_with_ctrl = Self::ToUnicodeString(
                    &key_state, vk, scancode, locale_identifier
                );
                Self::apply_mod_state(&mut key_state, Self::CONTROL_FLAG | Self::ALT_FLAG);
                let key_with_ctrl_alt = Self::ToUnicodeString(
                    &key_state, vk, scancode, locale_identifier
                );
                if key_with_ctrl.is_something() && key_with_ctrl_alt.is_something() {
                    self.has_alt_graph = key_with_ctrl != key_with_ctrl_alt;
                }
            }
        }
        self.known_locale_id = locale_identifier;
        true
    }

    fn ToUnicodeString(key_state: &[u8; 256], vkey: u32, scancode: u32, locale_identifier: usize) -> ToUnicodeResult {
        unsafe {
            let mut label_wide = [0u16; 8];
            let wide_len = winuser::ToUnicodeEx(
                vkey,
                scancode,
                (&key_state[0]) as *const _,
                (&mut label_wide[0]) as *mut _,
                label_wide.len() as i32,
                0,
                locale_identifier as _
            );
            if wide_len < 0 {
                // If it's dead, let's run `ToUnicode` again, to consume the dead-key
                winuser::ToUnicodeEx(
                    vkey,
                    scancode,
                    (&key_state[0]) as *const _,
                    (&mut label_wide[0]) as *mut _,
                    label_wide.len() as i32,
                    0,
                    locale_identifier as _
                );
                return ToUnicodeResult::Dead;
            }
            if wide_len > 0 {
                let os_string = OsString::from_wide(&label_wide[0..wide_len as usize]);
                if let Ok(label_str) = os_string.into_string() {
                    return ToUnicodeResult::Str(label_str);
                } else {
                    println!("Could not transform {:?}", label_wide);
                }
            }
        }
        ToUnicodeResult::None
    }

    fn apply_mod_state(key_state: &mut [u8; 256], mod_state: u8) {
        if mod_state & Self::SHIFT_FLAG != 0 {
            key_state[winuser::VK_SHIFT as usize] |= 0x80;
        } else {
            key_state[winuser::VK_SHIFT as usize] &= !0x80;
            key_state[winuser::VK_LSHIFT as usize] &= !0x80;
            key_state[winuser::VK_RSHIFT as usize] &= !0x80;
        }
        if mod_state & Self::CONTROL_FLAG != 0 {
            key_state[winuser::VK_CONTROL as usize] |= 0x80;
        } else {
            key_state[winuser::VK_CONTROL as usize] &= !0x80;
            key_state[winuser::VK_LCONTROL as usize] &= !0x80;
            key_state[winuser::VK_RCONTROL as usize] &= !0x80;
        }
        if mod_state & Self::ALT_FLAG != 0 {
            key_state[winuser::VK_MENU as usize] |= 0x80;
        } else {
            key_state[winuser::VK_MENU as usize] &= !0x80;
            key_state[winuser::VK_LMENU as usize] &= !0x80;
            key_state[winuser::VK_RMENU as usize] &= !0x80;
        }
        if mod_state & Self::CAPS_LOCK_FLAG != 0 {
            key_state[winuser::VK_CAPITAL as usize] |= 0x80;
        } else {
            key_state[winuser::VK_CAPITAL as usize] &= !0x80;
        }
    }
}

struct PartialKeyEventInfo {
    key_state: keyboard_types::KeyState,
    /// The native Virtual Key
    vkey: i32,
    scancode: PlatformScanCode,
    is_repeat: bool,
    code: keyboard_types::Code,
    location: keyboard_types::Location,
    /// True if the key event corresponds to a dead key input
    is_dead: bool,
    label: Option<String>,

    /// The utf16 code units of the text that was produced by the keypress event.
    /// This take all modifiers into account. Including CTRL
    utf16parts: Vec<u16>,

    utf16parts_without_ctrl: Vec<u16>,
}

impl PartialKeyEventInfo {
    fn finalize(self, locale_id: usize, has_alt_gr: bool) -> KeyEvent {
        let logical_key;
        if self.is_dead {
            logical_key = Key::Dead;
        } else {
            let key = vkey_to_non_printable(self.vkey, locale_id, has_alt_gr);
            match key {
                Key::Unidentified => {
                    if self.utf16parts_without_ctrl.len() > 0 {
                        logical_key = Key::Character(
                            String::from_utf16(&self.utf16parts_without_ctrl).unwrap()
                        );
                    } else {
                        logical_key = Key::Unidentified;
                    }
                }
                key @ _ => {
                    logical_key = key;
                }
            }
        }

        let key_without_modifers;
        if let Some(label) = &self.label {
            key_without_modifers = Key::Character(label.clone());
        } else {
            key_without_modifers = logical_key.clone();
        }

        let mut char_with_all_modifiers = None;
        if !self.utf16parts.is_empty() {
            char_with_all_modifiers = Some(String::from_utf16(&self.utf16parts).unwrap());
        }

        KeyEvent {
            scancode: ScanCode(self.scancode),
            physical_key: self.code,
            logical_key: logical_key,
            location: self.location,
            state: self.key_state,
            repeat: self.is_repeat,
            platform_specific: KeyEventExtra {
                char_with_all_modifers: char_with_all_modifiers,
                key_without_modifers: key_without_modifers,
            },
        }
    }
}

pub fn destructure_key_lparam(lparam: LPARAM) -> KeyLParam {
    let previous_state = (lparam >> 30) & 0x01;
    let transition_state = (lparam >> 31) & 0x01;
    KeyLParam {
        scancode: ((lparam >> 16) & 0xFF) as u8,
        extended: ((lparam >> 24) & 0x01) != 0,
        is_repeat: (previous_state ^ transition_state) != 0,
    }
}

pub fn get_location(
    vkey: c_int,
    extended: bool,
) -> keyboard_types::Location {
    use keyboard_types::{Code, Location};
    use winuser::*;
    const VK_ABNT_C2: c_int = 0xc2;

    // Use the native VKEY and the extended flag to cover most cases
    // This is taken from the `druid` software within
    // druid-shell/src/platform/windows/keyboard.rs
    match vkey {
        VK_LSHIFT | VK_LCONTROL | VK_LMENU | VK_LWIN => Location::Left,
        VK_RSHIFT | VK_RCONTROL | VK_RMENU | VK_RWIN => Location::Right,
        VK_RETURN if extended => Location::Numpad,
        VK_INSERT | VK_DELETE | VK_END | VK_DOWN | VK_NEXT | VK_LEFT | VK_CLEAR | VK_RIGHT
        | VK_HOME | VK_UP | VK_PRIOR => {
            if extended {
                Location::Standard
            } else {
                Location::Numpad
            }
        }
        VK_NUMPAD0 | VK_NUMPAD1 | VK_NUMPAD2 | VK_NUMPAD3 | VK_NUMPAD4 | VK_NUMPAD5
        | VK_NUMPAD6 | VK_NUMPAD7 | VK_NUMPAD8 | VK_NUMPAD9 | VK_DECIMAL | VK_DIVIDE
        | VK_MULTIPLY | VK_SUBTRACT | VK_ADD | VK_ABNT_C2 => Location::Numpad,
        _ => Location::Standard,
    }
}

unsafe fn get_utf16_without_ctrl(
    vkey: u32,
    scancode: PlatformScanCode,
    key_state: &mut [u8; 256],
    utf16parts_without_ctrl: &mut Vec<u16>
) {
    let target_capacity = 8;
    let curr_cap = utf16parts_without_ctrl.capacity();
    if curr_cap < target_capacity {
        utf16parts_without_ctrl.reserve(target_capacity - curr_cap);
    }
    // Now remove all CTRL stuff from the keyboard state
    key_state[winuser::VK_CONTROL as usize] = 0;
    key_state[winuser::VK_LCONTROL as usize] = 0;
    key_state[winuser::VK_RCONTROL as usize] = 0;
    let unicode_len = winuser::ToUnicode(
        vkey,
        scancode.0 as u32,
        (&mut key_state[0]) as *mut _,
        utf16parts_without_ctrl.as_mut_ptr(),
        utf16parts_without_ctrl.capacity() as i32,
        0
    );
    if unicode_len < 0 {
        utf16parts_without_ctrl.set_len(0);
    } else {
        utf16parts_without_ctrl.set_len(unicode_len as usize);
    }
}

// TODO: This list might not be complete
fn does_vkey_consume_dead_key(vkey: u32) -> bool {
    const A: u32 = 'A' as u32;
    const Z: u32 = 'Z' as u32;
    const ZERO: u32 = '0' as u32;
    const NINE: u32 = '9' as u32;
    match vkey {
        A..=Z | ZERO..=NINE => return true,
        _ => ()
    }
    match vkey as i32 {
        // OEM keys
        winuser::VK_OEM_1 | winuser::VK_OEM_2 | winuser::VK_OEM_3 | winuser::VK_OEM_4 |
        winuser::VK_OEM_5 | winuser::VK_OEM_6 | winuser::VK_OEM_7 | winuser::VK_OEM_8 |
        winuser::VK_OEM_PLUS | winuser::VK_OEM_COMMA | winuser::VK_OEM_MINUS |
        winuser::VK_OEM_PERIOD => {
            true
        }
        // Other keys
        winuser::VK_TAB | winuser::VK_BACK | winuser::VK_RETURN | winuser::VK_SPACE |
        winuser::VK_NUMPAD0..=winuser::VK_NUMPAD9 | winuser::VK_MULTIPLY | winuser::VK_ADD |
        winuser::VK_SUBTRACT | winuser::VK_DECIMAL | winuser::VK_DIVIDE => {
            true
        },
        _ => false,
    }
}

/// This includes all non-character keys defined within `Key` so for example
/// backspace and tab are included.
fn vkey_to_non_printable(vkey: i32, hkl: usize, has_alt_graph: bool) -> Key {
    // List of the Web key names and their corresponding platform-native key names:
    // https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/key/Key_Values

    let primary_lang_id = PRIMARYLANGID(LOWORD(hkl as u32));
    let is_korean = primary_lang_id == LANG_KOREAN;
    let is_japanese = primary_lang_id == LANG_JAPANESE;
    
    match vkey {
        winuser::VK_LBUTTON => Key::Unidentified, // Mouse
        winuser::VK_RBUTTON => Key::Unidentified, // Mouse
        winuser::VK_CANCEL => Key::Unidentified, // I don't think this can be represented with a Key
        winuser::VK_MBUTTON => Key::Unidentified, // Mouse
        winuser::VK_XBUTTON1 => Key::Unidentified, // Mouse
        winuser::VK_XBUTTON2 => Key::Unidentified, // Mouse
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

        // VK_HANGUL and VK_KANA are defined as the same constant therefore
        // we use appropriate conditions to differentate between them
        winuser::VK_HANGUL if is_korean => Key::HangulMode,
        winuser::VK_KANA if is_japanese => Key::KanaMode,

        winuser::VK_JUNJA => Key::JunjaMode,
        winuser::VK_FINAL => Key::FinalMode,

        // VK_HANJA and VK_KANJI are defined as the same constant therefore
        // we use appropriate conditions to differentate between them
        winuser::VK_HANJA if is_korean => Key::HanjaMode,
        winuser::VK_KANJI if is_japanese => Key::KanjiMode,

        winuser::VK_ESCAPE => Key::Escape,
        winuser::VK_CONVERT => Key::Convert,
        winuser::VK_NONCONVERT => Key::NonConvert,
        winuser::VK_ACCEPT => Key::Accept,
        winuser::VK_MODECHANGE => Key::ModeChange,
        winuser::VK_SPACE => Key::Unidentified, // This function only converts "non-printable"
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
        winuser::VK_LWIN => Key::Meta,
        winuser::VK_RWIN => Key::Meta,
        winuser::VK_APPS => Key::ContextMenu,
        winuser::VK_SLEEP => Key::Standby,

        // This function only converts "non-printable"
        winuser::VK_NUMPAD0 => Key::Unidentified,
        winuser::VK_NUMPAD1 => Key::Unidentified,
        winuser::VK_NUMPAD2 => Key::Unidentified,
        winuser::VK_NUMPAD3 => Key::Unidentified,
        winuser::VK_NUMPAD4 => Key::Unidentified,
        winuser::VK_NUMPAD5 => Key::Unidentified,
        winuser::VK_NUMPAD6 => Key::Unidentified,
        winuser::VK_NUMPAD7 => Key::Unidentified,
        winuser::VK_NUMPAD8 => Key::Unidentified,
        winuser::VK_NUMPAD9 => Key::Unidentified,
        winuser::VK_MULTIPLY => Key::Unidentified,
        winuser::VK_ADD => Key::Unidentified,
        winuser::VK_SEPARATOR => Key::Unidentified,
        winuser::VK_SUBTRACT => Key::Unidentified,
        winuser::VK_DECIMAL => Key::Unidentified,
        winuser::VK_DIVIDE => Key::Unidentified,

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

        // TODO: Uncomment when these are added to `keyboard_types`
        // winuser::VK_F13 => Key::F13,
        // winuser::VK_F14 => Key::F14,
        // winuser::VK_F15 => Key::F15,
        // winuser::VK_F16 => Key::F16,
        // winuser::VK_F17 => Key::F17,
        // winuser::VK_F18 => Key::F18,
        // winuser::VK_F19 => Key::F19,
        // winuser::VK_F20 => Key::F20,
        // winuser::VK_F21 => Key::F21,
        // winuser::VK_F22 => Key::F22,
        // winuser::VK_F23 => Key::F23,
        // winuser::VK_F24 => Key::F24,

        winuser::VK_NAVIGATION_VIEW => Key::Unidentified,
        winuser::VK_NAVIGATION_MENU => Key::Unidentified,
        winuser::VK_NAVIGATION_UP => Key::Unidentified,
        winuser::VK_NAVIGATION_DOWN => Key::Unidentified,
        winuser::VK_NAVIGATION_LEFT => Key::Unidentified,
        winuser::VK_NAVIGATION_RIGHT => Key::Unidentified,
        winuser::VK_NAVIGATION_ACCEPT => Key::Unidentified,
        winuser::VK_NAVIGATION_CANCEL => Key::Unidentified,
        winuser::VK_NUMLOCK => Key::NumLock,
        winuser::VK_SCROLL => Key::ScrollLock,
        winuser::VK_OEM_NEC_EQUAL => Key::Unidentified,
        //winuser::VK_OEM_FJ_JISHO => Key::Unidentified, // Conflicts with `VK_OEM_NEC_EQUAL`
        winuser::VK_OEM_FJ_MASSHOU => Key::Unidentified,
        winuser::VK_OEM_FJ_TOUROKU => Key::Unidentified,
        winuser::VK_OEM_FJ_LOYA => Key::Unidentified,
        winuser::VK_OEM_FJ_ROYA => Key::Unidentified,
        winuser::VK_LSHIFT => Key::Shift,
        winuser::VK_RSHIFT => Key::Shift,
        winuser::VK_LCONTROL => Key::Control,
        winuser::VK_RCONTROL => Key::Control,
        winuser::VK_LMENU => Key::Alt,
        winuser::VK_RMENU => {
            if has_alt_graph { Key::AltGraph }
            else { Key::Alt }
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
        winuser::VK_OEM_1 => Key::Unidentified,
        winuser::VK_OEM_PLUS => Key::Unidentified,
        winuser::VK_OEM_COMMA => Key::Unidentified,
        winuser::VK_OEM_MINUS => Key::Unidentified,
        winuser::VK_OEM_PERIOD => Key::Unidentified,
        winuser::VK_OEM_2 => Key::Unidentified,
        winuser::VK_OEM_3 => Key::Unidentified,

        winuser::VK_GAMEPAD_A => Key::Unidentified,
        winuser::VK_GAMEPAD_B => Key::Unidentified,
        winuser::VK_GAMEPAD_X => Key::Unidentified,
        winuser::VK_GAMEPAD_Y => Key::Unidentified,
        winuser::VK_GAMEPAD_RIGHT_SHOULDER => Key::Unidentified,
        winuser::VK_GAMEPAD_LEFT_SHOULDER => Key::Unidentified,
        winuser::VK_GAMEPAD_LEFT_TRIGGER => Key::Unidentified,
        winuser::VK_GAMEPAD_RIGHT_TRIGGER => Key::Unidentified,
        winuser::VK_GAMEPAD_DPAD_UP => Key::Unidentified,
        winuser::VK_GAMEPAD_DPAD_DOWN => Key::Unidentified,
        winuser::VK_GAMEPAD_DPAD_LEFT => Key::Unidentified,
        winuser::VK_GAMEPAD_DPAD_RIGHT => Key::Unidentified,
        winuser::VK_GAMEPAD_MENU => Key::Unidentified,
        winuser::VK_GAMEPAD_VIEW => Key::Unidentified,
        winuser::VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON => Key::Unidentified,
        winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON => Key::Unidentified,
        winuser::VK_GAMEPAD_LEFT_THUMBSTICK_UP => Key::Unidentified,
        winuser::VK_GAMEPAD_LEFT_THUMBSTICK_DOWN => Key::Unidentified,
        winuser::VK_GAMEPAD_LEFT_THUMBSTICK_RIGHT => Key::Unidentified,
        winuser::VK_GAMEPAD_LEFT_THUMBSTICK_LEFT => Key::Unidentified,
        winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_UP => Key::Unidentified,
        winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_DOWN => Key::Unidentified,
        winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_RIGHT => Key::Unidentified,
        winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT => Key::Unidentified,

        // This function only converts "non-printable"
        winuser::VK_OEM_4 => Key::Unidentified,
        winuser::VK_OEM_5 => Key::Unidentified,
        winuser::VK_OEM_6 => Key::Unidentified,
        winuser::VK_OEM_7 => Key::Unidentified,
        winuser::VK_OEM_8 => Key::Unidentified,
        winuser::VK_OEM_AX => Key::Unidentified,
        winuser::VK_OEM_102 => Key::Unidentified,

        winuser::VK_ICO_HELP => Key::Unidentified,
        winuser::VK_ICO_00 => Key::Unidentified,

        winuser::VK_PROCESSKEY => Key::Process,

        winuser::VK_ICO_CLEAR => Key::Unidentified,
        winuser::VK_PACKET => Key::Unidentified,
        winuser::VK_OEM_RESET => Key::Unidentified,
        winuser::VK_OEM_JUMP => Key::Unidentified,
        winuser::VK_OEM_PA1 => Key::Unidentified,
        winuser::VK_OEM_PA2 => Key::Unidentified,
        winuser::VK_OEM_PA3 => Key::Unidentified,
        winuser::VK_OEM_WSCTRL => Key::Unidentified,
        winuser::VK_OEM_CUSEL => Key::Unidentified,

        winuser::VK_OEM_ATTN => Key::Attn,
        winuser::VK_OEM_FINISH => {
            if is_japanese {
                Key::Katakana
            } else {
                // TODO: use Finish once that gets added to Key
                // Key::Finish
                Key::Unidentified
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
        winuser::VK_NONAME => Key::Unidentified,
        winuser::VK_PA1 => Key::Unidentified,
        winuser::VK_OEM_CLEAR => Key::Clear,
        _ => Key::Unidentified
    }
}

fn native_key_to_code(scancode: PlatformScanCode) -> keyboard_types::Code {
    // See: https://www.win.tue.nl/~aeb/linux/kbd/scancodes-1.html
    // and: https://www.w3.org/TR/uievents-code/
    // and: The widget/NativeKeyToDOMCodeName.h file in the firefox source

    use keyboard_types::Code;

    match scancode.0 {
        0x0029 => Code::Backquote,
        0x002B => Code::Backslash,
        0x000E => Code::Backspace,
        0x001A => Code::BracketLeft,
        0x001B => Code::BracketRight,
        0x0033 => Code::Comma,
        0x000B => Code::Digit0,
        0x0002 => Code::Digit1,
        0x0003 => Code::Digit2,
        0x0004 => Code::Digit3,
        0x0005 => Code::Digit4,
        0x0006 => Code::Digit5,
        0x0007 => Code::Digit6,
        0x0008 => Code::Digit7,
        0x0009 => Code::Digit8,
        0x000A => Code::Digit9,
        0x000D => Code::Equal,
        0x0056 => Code::IntlBackslash,
        0x0073 => Code::IntlRo,
        0x007D => Code::IntlYen,
        0x001E => Code::KeyA,
        0x0030 => Code::KeyB,
        0x002E => Code::KeyC,
        0x0020 => Code::KeyD,
        0x0012 => Code::KeyE,
        0x0021 => Code::KeyF,
        0x0022 => Code::KeyG,
        0x0023 => Code::KeyH,
        0x0017 => Code::KeyI,
        0x0024 => Code::KeyJ,
        0x0025 => Code::KeyK,
        0x0026 => Code::KeyL,
        0x0032 => Code::KeyM,
        0x0031 => Code::KeyN,
        0x0018 => Code::KeyO,
        0x0019 => Code::KeyP,
        0x0010 => Code::KeyQ,
        0x0013 => Code::KeyR,
        0x001F => Code::KeyS,
        0x0014 => Code::KeyT,
        0x0016 => Code::KeyU,
        0x002F => Code::KeyV,
        0x0011 => Code::KeyW,
        0x002D => Code::KeyX,
        0x0015 => Code::KeyY,
        0x002C => Code::KeyZ,
        0x000C => Code::Minus,
        0x0034 => Code::Period,
        0x0028 => Code::Quote,
        0x0027 => Code::Semicolon,
        0x0035 => Code::Slash,
        0x0038 => Code::AltLeft,
        0xE038 => Code::AltRight,
        0x003A => Code::CapsLock,
        0xE05D => Code::ContextMenu,
        0x001D => Code::ControlLeft,
        0xE01D => Code::ControlRight,
        0x001C => Code::Enter,
        //0xE05B => Code::OSLeft,
        //0xE05C => Code::OSRight,
        0x002A => Code::ShiftLeft,
        0x0036 => Code::ShiftRight,
        0x0039 => Code::Space,
        0x000F => Code::Tab,
        0x0079 => Code::Convert,
        0x0072 => Code::Lang1, // for non-Korean layout
        0xE0F2 => Code::Lang1, // for Korean layout
        0x0071 => Code::Lang2, // for non-Korean layout
        0xE0F1 => Code::Lang2, // for Korean layout
        0x0070 => Code::KanaMode,
        0x007B => Code::NonConvert,
        0xE053 => Code::Delete,
        0xE04F => Code::End,
        0xE047 => Code::Home,
        0xE052 => Code::Insert,
        0xE051 => Code::PageDown,
        0xE049 => Code::PageUp,
        0xE050 => Code::ArrowDown,
        0xE04B => Code::ArrowLeft,
        0xE04D => Code::ArrowRight,
        0xE048 => Code::ArrowUp,
        0xE045 => Code::NumLock,
        0x0052 => Code::Numpad0,
        0x004F => Code::Numpad1,
        0x0050 => Code::Numpad2,
        0x0051 => Code::Numpad3,
        0x004B => Code::Numpad4,
        0x004C => Code::Numpad5,
        0x004D => Code::Numpad6,
        0x0047 => Code::Numpad7,
        0x0048 => Code::Numpad8,
        0x0049 => Code::Numpad9,
        0x004E => Code::NumpadAdd,
        0x007E => Code::NumpadComma,
        0x0053 => Code::NumpadDecimal,
        0xE035 => Code::NumpadDivide,
        0xE01C => Code::NumpadEnter,
        0x0059 => Code::NumpadEqual,
        0x0037 => Code::NumpadMultiply,
        0x004A => Code::NumpadSubtract,
        0x0001 => Code::Escape,
        0x003B => Code::F1,
        0x003C => Code::F2,
        0x003D => Code::F3,
        0x003E => Code::F4,
        0x003F => Code::F5,
        0x0040 => Code::F6,
        0x0041 => Code::F7,
        0x0042 => Code::F8,
        0x0043 => Code::F9,
        0x0044 => Code::F10,
        0x0057 => Code::F11,
        0x0058 => Code::F12,
        // TODO: Add these when included in keyboard-types
        // 0x0064 => Code::F13,
        // 0x0065 => Code::F14,
        // 0x0066 => Code::F15,
        // 0x0067 => Code::F16,
        // 0x0068 => Code::F17,
        // 0x0069 => Code::F18,
        // 0x006A => Code::F19,
        // 0x006B => Code::F20,
        // 0x006C => Code::F21,
        // 0x006D => Code::F22,
        // 0x006E => Code::F23,
        // 0x0076 => Code::F24,
        0xE037 => Code::PrintScreen,
        0x0054 => Code::PrintScreen, // Alt + PrintScreen
        0x0046 => Code::ScrollLock,
        0x0045 => Code::Pause,
        0xE046 => Code::Pause, // Ctrl + Pause
        0xE06A => Code::BrowserBack,
        0xE066 => Code::BrowserFavorites,
        0xE069 => Code::BrowserForward,
        0xE032 => Code::BrowserHome,
        0xE067 => Code::BrowserRefresh,
        0xE065 => Code::BrowserSearch,
        0xE068 => Code::BrowserStop,
        0xE06B => Code::LaunchApp1,
        0xE021 => Code::LaunchApp2,
        0xE06C => Code::LaunchMail,
        0xE022 => Code::MediaPlayPause,
        0xE06D => Code::MediaSelect,
        0xE024 => Code::MediaStop,
        0xE019 => Code::MediaTrackNext,
        0xE010 => Code::MediaTrackPrevious,
        0xE05E => Code::Power,
        0xE02E => Code::AudioVolumeDown,
        0xE020 => Code::AudioVolumeMute,
        0xE030 => Code::AudioVolumeUp,
        _ => Code::Unidentified,
    }
}
