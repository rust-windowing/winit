use std::{
    char, collections::HashSet, ffi::OsString, mem::MaybeUninit, os::raw::c_int,
    os::windows::ffi::OsStringExt, sync::MutexGuard,
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
    event::{ElementState, KeyEvent},
    keyboard::{Key, KeyCode, KeyLocation, NativeKeyCode},
    platform::scancode::KeyCodeExtScancode,
    platform_impl::platform::{
        keyboard_layout::{get_or_insert_str, LayoutCache, WindowsModifiers, LAYOUT_CACHE},
        KeyEventExtra,
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

unsafe fn get_kbd_state() -> [u8; 256] {
    let mut kbd_state: [MaybeUninit<u8>; 256] = [MaybeUninit::uninit(); 256];
    winuser::GetKeyboardState(kbd_state[0].as_mut_ptr());
    std::mem::transmute::<_, [u8; 256]>(kbd_state)
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

                let mut layouts = LAYOUT_CACHE.lock().unwrap();
                let (event_info, _) =
                    PartialKeyEventInfo::from_message(lparam, ElementState::Pressed, &mut layouts);
                let mut event_info = Some(event_info);

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
                    let ev = event_info.finalize(&mut layouts.strings);
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
                let event_info = self.event_info.take().unwrap();
                *retval = Some(0);
                let mut layouts = LAYOUT_CACHE.lock().unwrap();
                let ev = event_info.finalize(&mut layouts.strings);
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
                    // Otherwise wparam holds a utf32 character.
                    // Let's encode it as utf16 appending it to the end of utf16parts
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

                    let mut layouts = LAYOUT_CACHE.lock().unwrap();
                    // Here it's okay to call `ToUnicode` because at this point the dead key
                    // is already consumed by the character.
                    unsafe {
                        let kbd_state = get_kbd_state();
                        let mod_state = WindowsModifiers::active_modifiers(&kbd_state);

                        let (_, layout) = layouts.get_current_layout();
                        let ctrl_on;
                        if layout.has_alt_graph {
                            let alt_on = mod_state.contains(WindowsModifiers::ALT);
                            ctrl_on = !alt_on && mod_state.contains(WindowsModifiers::CONTROL)
                        } else {
                            ctrl_on = mod_state.contains(WindowsModifiers::CONTROL)
                        }

                        // If CTRL is not pressed, just use the text with all
                        // modifiers because that already consumed the dead key and otherwise
                        // we would interpret the character incorretly, missing the dead key.
                        if !ctrl_on {
                            event_info.text = PartialText::System(event_info.utf16parts.clone());
                        } else {
                            let mod_no_ctrl = mod_state.remove_only_ctrl();
                            let scancode = event_info.scancode;
                            let keycode = event_info.code;
                            let key = layout.get_key(mod_no_ctrl, scancode, keycode);
                            event_info.text = PartialText::Text(key.to_text());
                        }
                    }
                    let ev = event_info.finalize(&mut layouts.strings);
                    return vec![MessageAsKeyEvent {
                        event: ev,
                        is_synthetic: false,
                    }];
                }
            }
            winuser::WM_KEYUP | winuser::WM_SYSKEYUP => {
                *retval = Some(0);

                let mut layouts = LAYOUT_CACHE.lock().unwrap();
                let (event_info, aux_info) =
                    PartialKeyEventInfo::from_message(lparam, ElementState::Released, &mut layouts);
                let logical_key = event_info.logical_key;
                let mut ev = event_info.finalize(&mut layouts.strings);
                let (_, layout) = layouts.get_current_layout();
                let key_with_all_modifiers =
                    layout.get_key(aux_info.modifiers, aux_info.scancode, aux_info.key_code);
                ev.text = logical_key.to_text();
                ev.platform_specific.char_with_all_modifers = key_with_all_modifiers.to_text();
                return vec![MessageAsKeyEvent {
                    event: ev,
                    is_synthetic: false,
                }];
            }
            _ => (),
        }

        Vec::new()
    }

    fn synthesize_kbd_state(&mut self, key_state: ElementState) -> Vec<MessageAsKeyEvent> {
        let mut key_events = Vec::new();

        let mut layouts = LAYOUT_CACHE.lock().unwrap();
        let (locale_id, _) = layouts.get_current_layout();

        let kbd_state = unsafe { get_kbd_state() };
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
            let event = self.create_synthetic(
                winuser::VK_CAPITAL,
                key_state,
                caps_lock_on,
                locale_id as HKL,
                &mut layouts,
            );
            if let Some(event) = event {
                key_events.push(event);
            }
        }
        let do_non_modifier = |key_events: &mut Vec<_>, layouts: &mut _| {
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
                let event =
                    self.create_synthetic(vk, key_state, caps_lock_on, locale_id as HKL, layouts);
                if let Some(event) = event {
                    key_events.push(event);
                }
            }
        };
        let do_modifier = |key_events: &mut Vec<_>, layouts: &mut _| {
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
                    let event = self.create_synthetic(
                        *vk,
                        key_state,
                        caps_lock_on,
                        locale_id as HKL,
                        layouts,
                    );
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
                do_non_modifier(&mut key_events, &mut layouts);
                do_modifier(&mut key_events, &mut layouts);
            }
            ElementState::Released => {
                do_modifier(&mut key_events, &mut layouts);
                do_non_modifier(&mut key_events, &mut layouts);
            }
        }

        key_events
    }

    fn create_synthetic(
        &self,
        vk: i32,
        key_state: ElementState,
        caps_lock_on: bool,
        locale_id: HKL,
        layouts: &mut MutexGuard<'_, LayoutCache>,
    ) -> Option<MessageAsKeyEvent> {
        let scancode = unsafe {
            winuser::MapVirtualKeyExW(vk as UINT, winuser::MAPVK_VK_TO_VSC_EX, locale_id)
        };
        if scancode == 0 {
            return None;
        }
        let scancode = scancode as ExScancode;
        let is_extended = (scancode & 0xE000) == 0xE000;
        let code = KeyCode::from_scancode(scancode as u32);
        let mods = if caps_lock_on {
            WindowsModifiers::CAPS_LOCK
        } else {
            WindowsModifiers::empty()
        };
        let logical_key;
        let key_without_modifers;
        {
            let layout = layouts.layouts.get(&(locale_id as u64)).unwrap();
            logical_key = layout.get_key(mods, scancode, code);
            key_without_modifers = layout.get_key(WindowsModifiers::empty(), scancode, code);
        }
        let event_info = PartialKeyEventInfo {
            logical_key,
            key_without_modifiers: key_without_modifers,
            key_state,
            scancode,
            is_repeat: false,
            code,
            location: get_location(vk, is_extended),
            utf16parts: Vec::with_capacity(8),
            text: PartialText::System(Vec::new()),
        };

        let mut event = event_info.finalize(&mut layouts.strings);
        event.logical_key = logical_key;
        event.platform_specific.char_with_all_modifers = logical_key.to_text();
        Some(MessageAsKeyEvent {
            event,
            is_synthetic: true,
        })
    }
}

enum PartialText {
    // Unicode
    System(Vec<u16>),
    Text(Option<&'static str>),
}

struct PartialKeyEventInfo {
    key_state: ElementState,
    scancode: ExScancode,
    is_repeat: bool,
    code: KeyCode,
    location: KeyLocation,
    logical_key: Key<'static>,

    key_without_modifiers: Key<'static>,

    /// The utf16 code units of the text that was produced by the keypress event.
    /// This take all modifiers into account. Including CTRL
    utf16parts: Vec<u16>,

    text: PartialText,
}

struct AuxKeyInfo {
    scancode: ExScancode,
    key_code: KeyCode,
    modifiers: WindowsModifiers,
}

impl PartialKeyEventInfo {
    fn from_message(
        lparam: LPARAM,
        state: ElementState,
        layouts: &mut MutexGuard<'_, LayoutCache>,
    ) -> (Self, AuxKeyInfo) {
        const NO_MODS: WindowsModifiers = WindowsModifiers::empty();

        let lparam_struct = destructure_key_lparam(lparam);
        let scancode = new_ex_scancode(lparam_struct.scancode, lparam_struct.extended);
        let code = KeyCode::from_scancode(scancode as u32);
        let vkey =
            unsafe { winuser::MapVirtualKeyW(scancode as u32, winuser::MAPVK_VSC_TO_VK_EX) as i32 };
        let location = get_location(vkey, lparam_struct.extended);

        let kbd_state = unsafe { get_kbd_state() };
        let mods = WindowsModifiers::active_modifiers(&kbd_state);
        let mods_without_ctrl = mods.remove_only_ctrl();

        let (_, layout) = layouts.get_current_layout();
        let logical_key = layout.get_key(mods_without_ctrl, scancode, code);
        let key_without_modifiers = match layout.get_key(NO_MODS, scancode, code) {
            // We convert dead keys into their character.
            // The reason for this is that `key_without_modifiers` is designed for key-bindings
            // but for example the US International treats `'` (apostrophe) as a dead key and
            // reguar US keyboard treats it a character. In order for a single binding configuration
            // to work with both layouts we forward each dead key as a character.
            Key::Dead(k) => {
                if let Some(ch) = k {
                    // I'm avoiding the heap allocation. I don't want to talk about it :(
                    let mut utf8 = [0; 4];
                    let s = ch.encode_utf8(&mut utf8);
                    let static_str = get_or_insert_str(&mut layouts.strings, s);
                    Key::Character(static_str)
                } else {
                    Key::Unidentified(NativeKeyCode::Unidentified)
                }
            }
            key => key,
        };
        let aux_info = AuxKeyInfo {
            scancode,
            key_code: code,
            modifiers: mods,
        };
        let partial_key_event_info = PartialKeyEventInfo {
            key_state: state,
            logical_key,
            key_without_modifiers,
            scancode,
            is_repeat: lparam_struct.is_repeat,
            code,
            location,
            utf16parts: Vec::with_capacity(8),
            text: PartialText::System(Vec::new()),
        };
        (partial_key_event_info, aux_info)
    }

    fn finalize(self, strings: &mut HashSet<&'static str>) -> KeyEvent {
        let mut char_with_all_modifiers = None;
        if !self.utf16parts.is_empty() {
            let os_string = OsString::from_wide(&self.utf16parts);
            if let Ok(string) = os_string.into_string() {
                let static_str = get_or_insert_str(strings, string);
                char_with_all_modifiers = Some(static_str);
            }
        }

        // The text without ctrl
        let mut text = None;
        match self.text {
            PartialText::System(wide) => {
                if !wide.is_empty() {
                    let os_string = OsString::from_wide(&wide);
                    if let Ok(string) = os_string.into_string() {
                        let static_str = get_or_insert_str(strings, string);
                        text = Some(static_str);
                    }
                }
            }
            PartialText::Text(s) => {
                text = s;
            }
        }

        KeyEvent {
            physical_key: self.code,
            logical_key: self.logical_key,
            text,
            location: self.location,
            state: self.key_state,
            repeat: self.is_repeat,
            platform_specific: KeyEventExtra {
                char_with_all_modifers: char_with_all_modifiers,
                key_without_modifers: self.key_without_modifiers,
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
