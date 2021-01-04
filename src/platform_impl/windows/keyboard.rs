use std::{
    char, collections::HashMap, ffi::OsString, fmt, mem::MaybeUninit, os::raw::c_int,
    os::windows::ffi::OsStringExt,
};

use winapi::{
    shared::{
        minwindef::{HKL, LOWORD, LPARAM, LRESULT, UINT, WPARAM},
        windef::HWND,
    },
    um::{
        winnt::{LANG_JAPANESE, LANG_KOREAN, PRIMARYLANGID},
        winuser,
    },
};

use crate::{
    event::{KeyEvent, ElementState},
    keyboard::{Key, KeyCode, NativeKeyCode, KeyLocation},
    platform_impl::platform::{
        keyboard_layout::LAYOUT_CACHE,
        event::KeyEventExtra
    },
};

pub fn is_msg_keyboard_related(msg: u32) -> bool {
    use winuser::{WM_KEYFIRST, WM_KEYLAST, WM_KILLFOCUS, WM_SETFOCUS};
    let is_keyboard_msg = WM_KEYFIRST <= msg && msg <= WM_KEYLAST;

    is_keyboard_msg || msg == WM_SETFOCUS || msg == WM_KILLFOCUS
}

#[derive(Debug, Copy, Clone)]
pub struct KeyLParam {
    pub scancode: u8,
    pub extended: bool,

    /// This is `previous_state XOR transition_state` see the lParam for WM_KEYDOWN and WM_KEYUP.
    pub is_repeat: bool,
}

pub type ExScancode = u16;

fn new_ex_scancode(scancode: u8, extended: bool) -> ExScancode {
    (scancode as u16) | (if extended { 0xE000 } else { 0 })
}

pub struct MessageAsKeyEvent {
    pub event: KeyEvent,
    pub is_synthetic: bool,
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
            prev_down_was_dead: false,
        }
    }
}
impl KeyEventBuilder {
    /// Call this function for every window message.
    /// Returns Some() if this window message completes a KeyEvent.
    /// Returns None otherwise.
    pub fn process_message(
        &mut self,
        hwnd: HWND,
        msg_kind: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        retval: &mut Option<LRESULT>,
    ) -> Vec<MessageAsKeyEvent> {
        match msg_kind {
            winuser::WM_SETFOCUS => {
                // synthesize keydown events
                let key_events = self.synthesize_kbd_state(ElementState::Pressed);
                if !key_events.is_empty() {
                    return key_events;
                }
            }
            winuser::WM_KILLFOCUS => {
                // sythesize keyup events
                let key_events = self.synthesize_kbd_state(ElementState::Released);
                if !key_events.is_empty() {
                    return key_events;
                }
            }
            winuser::WM_KEYDOWN | winuser::WM_SYSKEYDOWN => {
                if msg_kind == winuser::WM_SYSKEYDOWN && wparam as i32 == winuser::VK_F4 {
                    // Don't dispatch Alt+F4 to the application.
                    // This is handled in `event_loop.rs`
                    return vec![];
                }
                self.prev_down_was_dead = false;

                let layouts = LAYOUT_CACHE.lock().unwrap();
                let (locale_id, layout) = layouts.get_current_layout();

                //let vkey = wparam as i32;

                let lparam_struct = destructure_key_lparam(lparam);
                let scancode =
                    new_ex_scancode(lparam_struct.scancode, lparam_struct.extended);
                let code = native_key_to_code(scancode);
                let vkey = unsafe {
                    winuser::MapVirtualKeyW(scancode as u32, winuser::MAPVK_VSC_TO_VK_EX) as i32
                };
                let location = get_location(vkey, lparam_struct.extended);
                let label = self.key_text.get(&scancode).map(|s| s.clone());
                let mut event_info = Some(PartialKeyEventInfo {
                    key_state: ElementState::Pressed,
                    vkey,
                    scancode,
                    is_repeat: lparam_struct.is_repeat,
                    code,
                    location,
                    is_dead: false,
                    label,
                    utf16parts: Vec::with_capacity(8),
                    utf16parts_without_ctrl: Vec::with_capacity(8),
                });

                let mut next_msg = MaybeUninit::uninit();
                let peek_retval = unsafe {
                    winuser::PeekMessageW(
                        next_msg.as_mut_ptr(),
                        hwnd,
                        winuser::WM_KEYFIRST,
                        winuser::WM_KEYLAST,
                        winuser::PM_NOREMOVE,
                    )
                };
                let has_next_key_message = peek_retval != 0;
                *retval = Some(0);
                self.event_info = None;
                if has_next_key_message {
                    let next_msg = unsafe { next_msg.assume_init().message };
                    let is_next_keydown =
                        next_msg == winuser::WM_KEYDOWN || next_msg == winuser::WM_SYSKEYDOWN;
                    if !is_next_keydown {
                        self.event_info = event_info.take();
                    }
                }
                if let Some(event_info) = event_info {
                    let ev = event_info.finalize(locale_id as usize, self.has_alt_graph);
                    return vec![MessageAsKeyEvent {
                        event: ev,
                        is_synthetic: false,
                    }];
                }
            }
            winuser::WM_DEADCHAR | winuser::WM_SYSDEADCHAR => {
                self.prev_down_was_dead = true;
                // At this point we know that there isn't going to be any more events related to
                // this key press
                let mut event_info = self.event_info.take().unwrap();
                event_info.is_dead = true;
                *retval = Some(0);
                let ev = event_info.finalize(self.known_locale_id, self.has_alt_graph);
                return vec![MessageAsKeyEvent {
                    event: ev,
                    is_synthetic: false,
                }];
            }
            winuser::WM_CHAR | winuser::WM_SYSCHAR => {
                *retval = Some(0);
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
                        winuser::PM_NOREMOVE,
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
                    self.event_info
                        .as_mut()
                        .unwrap()
                        .utf16parts
                        .push(wparam as u16);
                } else {
                    let utf16parts = &mut self.event_info.as_mut().unwrap().utf16parts;
                    let start_offset = utf16parts.len();
                    let new_size = utf16parts.len() + 2;
                    utf16parts.resize(new_size, 0);
                    if let Some(ch) = char::from_u32(wparam as u32) {
                        let encode_len = ch.encode_utf16(&mut utf16parts[start_offset..]).len();
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

                        let has_ctrl = key_state[winuser::VK_CONTROL as usize] & 0x80 != 0
                            || key_state[winuser::VK_LCONTROL as usize] & 0x80 != 0
                            || key_state[winuser::VK_RCONTROL as usize] & 0x80 != 0;

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
                                &mut event_info.utf16parts_without_ctrl,
                            );
                        }
                    }

                    let ev = event_info.finalize(self.known_locale_id, self.has_alt_graph);
                    return vec![MessageAsKeyEvent {
                        event: ev,
                        is_synthetic: false,
                    }];
                }
            }
            winuser::WM_KEYUP | winuser::WM_SYSKEYUP => {
                *retval = Some(0);
                let lparam_struct = destructure_key_lparam(lparam);
                let scancode =
                    PlatformScanCode::new(lparam_struct.scancode, lparam_struct.extended);
                let code = native_key_to_code(scancode);
                let vkey = unsafe {
                    winuser::MapVirtualKeyW(scancode.0 as u32, winuser::MAPVK_VSC_TO_VK_EX) as i32
                };
                let location = get_location(vkey, lparam_struct.extended);
                let mut utf16parts = Vec::with_capacity(8);
                let mut utf16parts_without_ctrl = Vec::with_capacity(8);

                // Avoid calling `ToUnicode` (which resets dead keys) if either the event
                // belongs to the key-down which just produced the dead key or if
                // the current key would not otherwise reset the dead key state.
                //
                // This logic relies on the assuption that keys which don't consume
                // dead keys, also do not produce text input.
                if !self.prev_down_was_dead && vkey_consumes_dead_key(wparam as u32) {
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
                            0,
                        );
                        utf16parts.set_len(unicode_len as usize);

                        get_utf16_without_ctrl(
                            wparam as u32,
                            scancode,
                            &mut key_state,
                            &mut utf16parts_without_ctrl,
                        );
                    }
                }
                let label = self.key_text.get(&scancode).map(|s| s.clone());
                let event_info = PartialKeyEventInfo {
                    key_state: ElementState::Released,
                    vkey,
                    scancode,
                    is_repeat: false,
                    code,
                    location,
                    is_dead: self.prev_down_was_dead,
                    label,
                    utf16parts,
                    utf16parts_without_ctrl,
                };
                let ev = event_info.finalize(self.known_locale_id, self.has_alt_graph);
                return vec![MessageAsKeyEvent {
                    event: ev,
                    is_synthetic: false,
                }];
            }
            _ => (),
        }

        Vec::new()
    }

    fn synthesize_kbd_state(
        &mut self,
        key_state: ElementState,
    ) -> Vec<MessageAsKeyEvent> {
        let mut key_events = Vec::new();
        let locale_id = unsafe { winuser::GetKeyboardLayout(0) };
        self.prepare_layout(locale_id as usize);

        let kbd_state = unsafe {
            let mut kbd_state: [MaybeUninit<u8>; 256] = [MaybeUninit::uninit(); 256];
            winuser::GetKeyboardState(kbd_state[0].as_mut_ptr());
            std::mem::transmute::<_, [u8; 256]>(kbd_state)
        };
        macro_rules! is_key_pressed {
            ($vk:expr) => {
                kbd_state[$vk as usize] & 0x80 != 0
            };
        }

        // Is caps-lock active? Be careful that this is different from caps-lock
        // being held down.
        let caps_lock_on = kbd_state[winuser::VK_CAPITAL as usize] & 1 != 0;

        // We are synthesizing the press event for caps-lock first. The reason:
        // 1, if caps-lock is *not* held down but it's active, than we have to
        // synthesize all printable keys, respecting the calps-lock state
        // 2, if caps-lock is held down, we could choose to sythesize it's
        // keypress after every other key, in which case all other keys *must*
        // be sythesized as if the caps-lock state would be the opposite
        // of what it currently is.
        // --
        // For the sake of simplicity we are choosing to always sythesize
        // caps-lock first, and always use the current caps-lock state
        // to determine the produced text
        if is_key_pressed!(winuser::VK_CAPITAL) {
            let event =
                self.create_synthetic(winuser::VK_CAPITAL, key_state, locale_id, caps_lock_on);
            if let Some(event) = event {
                key_events.push(event);
            }
        }
        let do_non_modifier = |key_events: &mut Vec<_>| {
            for vk in 0..256 {
                match vk {
                    winuser::VK_CONTROL
                    | winuser::VK_LCONTROL
                    | winuser::VK_RCONTROL
                    | winuser::VK_SHIFT
                    | winuser::VK_LSHIFT
                    | winuser::VK_RSHIFT
                    | winuser::VK_MENU
                    | winuser::VK_LMENU
                    | winuser::VK_RMENU
                    | winuser::VK_CAPITAL => continue,
                    _ => (),
                }
                if !is_key_pressed!(vk) {
                    continue;
                }
                if let Some(event) = self.create_synthetic(vk, key_state, locale_id, caps_lock_on) {
                    key_events.push(event);
                }
            }
        };
        let do_modifier = |key_events: &mut Vec<_>| {
            const CLEAR_MODIFIER_VKS: [i32; 6] = [
                winuser::VK_LCONTROL,
                winuser::VK_LSHIFT,
                winuser::VK_LMENU,
                winuser::VK_RCONTROL,
                winuser::VK_RSHIFT,
                winuser::VK_RMENU,
            ];
            for vk in CLEAR_MODIFIER_VKS.iter() {
                if is_key_pressed!(*vk) {
                    let event = self.create_synthetic(*vk, key_state, locale_id, caps_lock_on);
                    if let Some(event) = event {
                        key_events.push(event);
                    }
                }
            }
        };

        // Be cheeky and sequence modifier and non-modifier
        // key events such that non-modifier keys are not affected
        // by modifiers (except for caps-lock)
        match key_state {
            ElementState::Pressed => {
                do_non_modifier(&mut key_events);
                do_modifier(&mut key_events);
            }
            ElementState::Released => {
                do_modifier(&mut key_events);
                do_non_modifier(&mut key_events);
            }
        }

        key_events
    }

    fn create_synthetic(
        &self,
        vk: i32,
        key_state: ElementState,
        locale_id: HKL,
        caps_lock_on: bool,
    ) -> Option<MessageAsKeyEvent> {
        let scancode = unsafe {
            winuser::MapVirtualKeyExW(vk as UINT, winuser::MAPVK_VK_TO_VSC_EX, locale_id)
        };
        if scancode == 0 {
            return None;
        }
        let scancode = scancode as ExScancode;
        let is_extended = (scancode & 0xE000) == 0xE000;
        let code = native_key_to_code(scancode);
        let key_text = self.key_text.get(&scancode).cloned();
        let key_text_with_caps = self.key_text_with_caps.get(&scancode).cloned();
        let logical_key = match &key_text {
            Some(str) => {
                if caps_lock_on {
                    match key_text_with_caps.clone() {
                        Some(str) => keyboard_types::Key::Character(str),
                        None => keyboard_types::Key::Unidentified(native_code),
                    }
                } else {
                    keyboard_types::Key::Character(str.clone())
                }
            }
            None => vkey_to_non_printable(vk, code, locale_id as usize, self.has_alt_graph),
        };

        let event_info = PartialKeyEventInfo {
            key_state,
            vkey: vk,
            scancode: platform_scancode,
            is_repeat: false,
            code,
            location: get_location(vk, is_extended),
            is_dead: false,
            label: key_text,
            utf16parts: Vec::with_capacity(8),
            utf16parts_without_ctrl: Vec::with_capacity(8),
        };

        let mut event = event_info.finalize(locale_id as usize, self.has_alt_graph);
        event.logical_key = logical_key;
        event.platform_specific.char_with_all_modifers = key_text_with_caps;
        Some(MessageAsKeyEvent {
            event,
            is_synthetic: true,
        })
    }
}

struct PartialKeyEventInfo {
    key_state: ElementState,
    /// The native Virtual Key
    vkey: i32,
    scancode: ExScancode,
    is_repeat: bool,
    code: KeyCode,
    location: KeyLocation,
    logical_key: Key<'static>,

    /// The utf16 code units of the text that was produced by the keypress event.
    /// This take all modifiers into account. Including CTRL
    utf16parts: Vec<u16>,

    utf16parts_without_ctrl: Vec<u16>,
}

impl PartialKeyEventInfo {
    fn finalize(self, locale_id: usize, has_alt_gr: bool) -> KeyEvent {
        let logical_key;
        if self.is_dead {
            // TODO: dispatch the dead-key char here
            logical_key = Key::Dead(None);
        } else {
            if !self.utf16parts_without_ctrl.is_empty() {
                let string = String::from_utf16(&self.utf16parts_without_ctrl).unwrap();
                // TODO: cache these in a global map
                let leaked = Box::leak(Box::<str>::from(string));
                logical_key = Key::Character(leaked);
            } else {
                logical_key = vkey_to_non_printable(self.vkey, self.code, locale_id, has_alt_gr);
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
            physical_key: self.code,
            logical_key,
            text: None,
            location: self.location,
            state: self.key_state,
            repeat: self.is_repeat,
            platform_specific: KeyEventExtra {
                char_with_all_modifers: char_with_all_modifiers,
                key_without_modifers,
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

pub fn get_location(vkey: c_int, extended: bool) -> KeyLocation {
    use winuser::*;
    const VK_ABNT_C2: c_int = 0xc2;

    // Use the native VKEY and the extended flag to cover most cases
    // This is taken from the `druid` software within
    // druid-shell/src/platform/windows/keyboard.rs
    match vkey {
        VK_LSHIFT | VK_LCONTROL | VK_LMENU | VK_LWIN => KeyLocation::Left,
        VK_RSHIFT | VK_RCONTROL | VK_RMENU | VK_RWIN => KeyLocation::Right,
        VK_RETURN if extended => KeyLocation::Numpad,
        VK_INSERT | VK_DELETE | VK_END | VK_DOWN | VK_NEXT | VK_LEFT | VK_CLEAR | VK_RIGHT
        | VK_HOME | VK_UP | VK_PRIOR => {
            if extended {
                KeyLocation::Standard
            } else {
                KeyLocation::Numpad
            }
        }
        VK_NUMPAD0 | VK_NUMPAD1 | VK_NUMPAD2 | VK_NUMPAD3 | VK_NUMPAD4 | VK_NUMPAD5
        | VK_NUMPAD6 | VK_NUMPAD7 | VK_NUMPAD8 | VK_NUMPAD9 | VK_DECIMAL | VK_DIVIDE
        | VK_MULTIPLY | VK_SUBTRACT | VK_ADD | VK_ABNT_C2 => KeyLocation::Numpad,
        _ => KeyLocation::Standard,
    }
}

unsafe fn get_utf16_without_ctrl(
    vkey: u32,
    scancode: ExScancode,
    key_state: &mut [u8; 256],
    utf16parts_without_ctrl: &mut Vec<u16>,
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
        scancode as u32,
        (&mut key_state[0]) as *mut _,
        utf16parts_without_ctrl.as_mut_ptr(),
        utf16parts_without_ctrl.capacity() as i32,
        0,
    );
    if unicode_len < 0 {
        utf16parts_without_ctrl.set_len(0);
    } else {
        utf16parts_without_ctrl.set_len(unicode_len as usize);
    }
}

// TODO: This list might not be complete
fn vkey_consumes_dead_key(vkey: u32) -> bool {
    const A: u32 = 'A' as u32;
    const Z: u32 = 'Z' as u32;
    const ZERO: u32 = '0' as u32;
    const NINE: u32 = '9' as u32;
    match vkey {
        A..=Z | ZERO..=NINE => return true,
        _ => (),
    }
    match vkey as i32 {
        // OEM keys
        winuser::VK_OEM_1
        | winuser::VK_OEM_2
        | winuser::VK_OEM_3
        | winuser::VK_OEM_4
        | winuser::VK_OEM_5
        | winuser::VK_OEM_6
        | winuser::VK_OEM_7
        | winuser::VK_OEM_8
        | winuser::VK_OEM_PLUS
        | winuser::VK_OEM_COMMA
        | winuser::VK_OEM_MINUS
        | winuser::VK_OEM_PERIOD => true,
        // Other keys
        winuser::VK_TAB
        | winuser::VK_BACK
        | winuser::VK_RETURN
        | winuser::VK_SPACE
        | winuser::VK_NUMPAD0..=winuser::VK_NUMPAD9
        | winuser::VK_MULTIPLY
        | winuser::VK_ADD
        | winuser::VK_SUBTRACT
        | winuser::VK_DECIMAL
        | winuser::VK_DIVIDE => true,
        _ => false,
    }
}

/// This includes all non-character keys defined within `Key` so for example
/// backspace and tab are included.
pub fn vkey_to_non_printable(
    vkey: i32,
    native_code: NativeKeyCode,
    code: KeyCode,
    hkl: u64,
    has_alt_graph: bool,
) -> Key<'static> {
    // List of the Web key names and their corresponding platform-native key names:
    // https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/key/Key_Values

    // Some keys cannot be correctly determined based on the virtual key.
    // Therefore we use the `code` to translate those keys.
    match code {
        KeyCode::NumLock => return Key::NumLock,
        KeyCode::Pause => return Key::Pause,
        _ => (),
    }

    let primary_lang_id = PRIMARYLANGID(LOWORD(hkl as u32));
    let is_korean = primary_lang_id == LANG_KOREAN;
    let is_japanese = primary_lang_id == LANG_JAPANESE;

    match vkey {
        winuser::VK_LBUTTON => Key::Unidentified(NativeKeyCode::Unidentified),  // Mouse
        winuser::VK_RBUTTON => Key::Unidentified(NativeKeyCode::Unidentified),  // Mouse

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
        winuser::VK_SPACE => Key::Unidentified(native_code), // This function only converts "non-printable"
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
                // At the time of writing there is no `Key::Finish` variant as
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

pub fn native_key_to_code(scancode: ExScancode) -> KeyCode {
    // See: https://www.win.tue.nl/~aeb/linux/kbd/scancodes-1.html
    // and: https://www.w3.org/TR/uievents-code/
    // and: The widget/NativeKeyToDOMCodeName.h file in the firefox source

    match scancode {
        0x0029 => KeyCode::Backquote,
        0x002B => KeyCode::Backslash,
        0x000E => KeyCode::Backspace,
        0x001A => KeyCode::BracketLeft,
        0x001B => KeyCode::BracketRight,
        0x0033 => KeyCode::Comma,
        0x000B => KeyCode::Digit0,
        0x0002 => KeyCode::Digit1,
        0x0003 => KeyCode::Digit2,
        0x0004 => KeyCode::Digit3,
        0x0005 => KeyCode::Digit4,
        0x0006 => KeyCode::Digit5,
        0x0007 => KeyCode::Digit6,
        0x0008 => KeyCode::Digit7,
        0x0009 => KeyCode::Digit8,
        0x000A => KeyCode::Digit9,
        0x000D => KeyCode::Equal,
        0x0056 => KeyCode::IntlBackslash,
        0x0073 => KeyCode::IntlRo,
        0x007D => KeyCode::IntlYen,
        0x001E => KeyCode::KeyA,
        0x0030 => KeyCode::KeyB,
        0x002E => KeyCode::KeyC,
        0x0020 => KeyCode::KeyD,
        0x0012 => KeyCode::KeyE,
        0x0021 => KeyCode::KeyF,
        0x0022 => KeyCode::KeyG,
        0x0023 => KeyCode::KeyH,
        0x0017 => KeyCode::KeyI,
        0x0024 => KeyCode::KeyJ,
        0x0025 => KeyCode::KeyK,
        0x0026 => KeyCode::KeyL,
        0x0032 => KeyCode::KeyM,
        0x0031 => KeyCode::KeyN,
        0x0018 => KeyCode::KeyO,
        0x0019 => KeyCode::KeyP,
        0x0010 => KeyCode::KeyQ,
        0x0013 => KeyCode::KeyR,
        0x001F => KeyCode::KeyS,
        0x0014 => KeyCode::KeyT,
        0x0016 => KeyCode::KeyU,
        0x002F => KeyCode::KeyV,
        0x0011 => KeyCode::KeyW,
        0x002D => KeyCode::KeyX,
        0x0015 => KeyCode::KeyY,
        0x002C => KeyCode::KeyZ,
        0x000C => KeyCode::Minus,
        0x0034 => KeyCode::Period,
        0x0028 => KeyCode::Quote,
        0x0027 => KeyCode::Semicolon,
        0x0035 => KeyCode::Slash,
        0x0038 => KeyCode::AltLeft,
        0xE038 => KeyCode::AltRight,
        0x003A => KeyCode::CapsLock,
        0xE05D => KeyCode::ContextMenu,
        0x001D => KeyCode::ControlLeft,
        0xE01D => KeyCode::ControlRight,
        0x001C => KeyCode::Enter,
        //0xE05B => KeyCode::OSLeft,
        //0xE05C => KeyCode::OSRight,
        0x002A => KeyCode::ShiftLeft,
        0x0036 => KeyCode::ShiftRight,
        0x0039 => KeyCode::Space,
        0x000F => KeyCode::Tab,
        0x0079 => KeyCode::Convert,
        0x0072 => KeyCode::Lang1, // for non-Korean layout
        0xE0F2 => KeyCode::Lang1, // for Korean layout
        0x0071 => KeyCode::Lang2, // for non-Korean layout
        0xE0F1 => KeyCode::Lang2, // for Korean layout
        0x0070 => KeyCode::KanaMode,
        0x007B => KeyCode::NonConvert,
        0xE053 => KeyCode::Delete,
        0xE04F => KeyCode::End,
        0xE047 => KeyCode::Home,
        0xE052 => KeyCode::Insert,
        0xE051 => KeyCode::PageDown,
        0xE049 => KeyCode::PageUp,
        0xE050 => KeyCode::ArrowDown,
        0xE04B => KeyCode::ArrowLeft,
        0xE04D => KeyCode::ArrowRight,
        0xE048 => KeyCode::ArrowUp,
        0xE045 => KeyCode::NumLock,
        0x0052 => KeyCode::Numpad0,
        0x004F => KeyCode::Numpad1,
        0x0050 => KeyCode::Numpad2,
        0x0051 => KeyCode::Numpad3,
        0x004B => KeyCode::Numpad4,
        0x004C => KeyCode::Numpad5,
        0x004D => KeyCode::Numpad6,
        0x0047 => KeyCode::Numpad7,
        0x0048 => KeyCode::Numpad8,
        0x0049 => KeyCode::Numpad9,
        0x004E => KeyCode::NumpadAdd,
        0x007E => KeyCode::NumpadComma,
        0x0053 => KeyCode::NumpadDecimal,
        0xE035 => KeyCode::NumpadDivide,
        0xE01C => KeyCode::NumpadEnter,
        0x0059 => KeyCode::NumpadEqual,
        0x0037 => KeyCode::NumpadMultiply,
        0x004A => KeyCode::NumpadSubtract,
        0x0001 => KeyCode::Escape,
        0x003B => KeyCode::F1,
        0x003C => KeyCode::F2,
        0x003D => KeyCode::F3,
        0x003E => KeyCode::F4,
        0x003F => KeyCode::F5,
        0x0040 => KeyCode::F6,
        0x0041 => KeyCode::F7,
        0x0042 => KeyCode::F8,
        0x0043 => KeyCode::F9,
        0x0044 => KeyCode::F10,
        0x0057 => KeyCode::F11,
        0x0058 => KeyCode::F12,
        0x0064 => KeyCode::F13,
        0x0065 => KeyCode::F14,
        0x0066 => KeyCode::F15,
        0x0067 => KeyCode::F16,
        0x0068 => KeyCode::F17,
        0x0069 => KeyCode::F18,
        0x006A => KeyCode::F19,
        0x006B => KeyCode::F20,
        0x006C => KeyCode::F21,
        0x006D => KeyCode::F22,
        0x006E => KeyCode::F23,
        0x0076 => KeyCode::F24,
        0xE037 => KeyCode::PrintScreen,
        0x0054 => KeyCode::PrintScreen, // Alt + PrintScreen
        0x0046 => KeyCode::ScrollLock,
        0x0045 => KeyCode::Pause,
        0xE046 => KeyCode::Pause, // Ctrl + Pause
        0xE06A => KeyCode::BrowserBack,
        0xE066 => KeyCode::BrowserFavorites,
        0xE069 => KeyCode::BrowserForward,
        0xE032 => KeyCode::BrowserHome,
        0xE067 => KeyCode::BrowserRefresh,
        0xE065 => KeyCode::BrowserSearch,
        0xE068 => KeyCode::BrowserStop,
        0xE06B => KeyCode::LaunchApp1,
        0xE021 => KeyCode::LaunchApp2,
        0xE06C => KeyCode::LaunchMail,
        0xE022 => KeyCode::MediaPlayPause,
        0xE06D => KeyCode::MediaSelect,
        0xE024 => KeyCode::MediaStop,
        0xE019 => KeyCode::MediaTrackNext,
        0xE010 => KeyCode::MediaTrackPrevious,
        0xE05E => KeyCode::Power,
        0xE02E => KeyCode::AudioVolumeDown,
        0xE020 => KeyCode::AudioVolumeMute,
        0xE030 => KeyCode::AudioVolumeUp,
        _ => KeyCode::Unidentified(NativeKeyCode::Windows(scancode)),
    }
}
