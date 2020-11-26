use std::{os::raw::c_int, fmt, collections::HashMap};

use winapi::{shared::{minwindef::{LRESULT, LPARAM, WPARAM}, windef::HWND}, um::winuser, um::winnls};

use std::{char, mem::MaybeUninit, ffi::OsString, os::windows::ffi::OsStringExt};

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
    /// change the keyboard state (it removes the dead key). There is a flag to prevent
    /// changing the state but that flag requires Windows 10, version 1607 or newer)
    key_labels: HashMap<PlatformScanCode, String>,

    /// The locale identifier (HKL) of the layout for which the `key_labels` was generated
    known_locale_id: usize,

    /// The keyup event needs to call `ToUnicode` to determine key with all modifiers except CTRL
    /// (the `logical_key`).
    /// 
    /// But `ToUnicode` without the non-modifying flag (see `key_labels`), resets the dead key
    /// state which would be incorrect during every keyup event. Therefore this variable is used
    /// to determine whether the last keydown event produced a dead key.
    ///
    /// Note that this variable is not always correct because it does
    /// not track key presses outside of this window. However the ONLY situation where this
    /// doesn't work as intended is when the user presses a dead key outside of this window, and
    /// switched to this window BEFORE releasing it then releases the dead key. In this case
    /// the `ToUnicode` function will be called, incorrectly clearing the dead key state. Having
    /// an inccorect behaviour in this case seems acceptable.
    prev_down_was_dead: bool,
}
impl Default for KeyEventBuilder {
    fn default() -> Self {
        KeyEventBuilder {
            event_info: None,
            key_labels: HashMap::with_capacity(128),
            known_locale_id: 0,
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
                self.generate_labels(locale_id as usize);

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
                return Some(event_info.finalize());
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

                    return Some(event_info.finalize());
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
                return Some(event_info.finalize());
            }
            _ => ()
        }

        None
    }

    /// Returns true if succeeded.
    fn generate_labels(&mut self, locale_identifier: usize) -> bool {
        println!("Generating labels");
        if self.known_locale_id == locale_identifier {
            println!("Skipping generation because locales are identical");
            return true;
        }

        // We initialize the keyboard state with all zeros to
        // simulate a scenario when no modifier is active.
        let mut key_state = [0u8; 256];
        self.key_labels.clear();
        unsafe {
            let mut add_key_label = |vkey: u32| {
                let scancode = winuser::MapVirtualKeyExW(
                    vkey,
                    winuser::MAPVK_VK_TO_VSC_EX,
                    locale_identifier as _
                );
                if scancode == 0 {
                    return;
                }
                let platform_scancode = PlatformScanCode(scancode as u16);

                let mut label_wide = [0u16; 8];
                let wide_len = winuser::ToUnicodeEx(
                    vkey,
                    scancode,
                    (&mut key_state[0]) as *mut _,
                    (&mut label_wide[0]) as *mut _,
                    label_wide.len() as i32,
                    0,
                    locale_identifier as _
                );
                if wide_len > 0 {
                    let os_string = OsString::from_wide(&label_wide[0..wide_len as usize]);
                    if let Ok(label_str) = os_string.into_string() {
                        self.key_labels.insert(platform_scancode, label_str);
                    } else {
                        println!("Could not transform {:?}", label_wide);
                    }
                }
            };
            for &vk in VIRTUAL_KEY_ENUMS {
                add_key_label(vk as u32);
            }
            for ch in 'A'..='Z' {
                let vk = ch as u32;
                add_key_label(vk);
            }
            for ch in '0'..='9' {
                let vk = ch as u32;
                add_key_label(vk);
            }
        }
        self.known_locale_id = locale_identifier;
        println!("{}, {}", file!(), line!());
        true
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
    fn finalize(self) -> KeyEvent {
        use keyboard_types::Key;

        let logical_key;
        if self.is_dead {
            logical_key = Key::Dead;
        } else {
            // TODO: translate non-printable keys to `Key`
            if self.utf16parts_without_ctrl.len() > 0 {
                logical_key =
                    Key::Character(String::from_utf16(&self.utf16parts_without_ctrl).unwrap());
            } else {
                logical_key = Key::Unidentified;
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

pub fn native_key_to_code(scancode: PlatformScanCode) -> keyboard_types::Code {
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

/// Warning: this does not cover all possible virtual keys.
/// Most notably it does not cover [A, Z] and [0, 9]
/// Those each have the value of ther corresponding uppercase `char`.
/// E.g. the virtual key A has the value `'A'`
const VIRTUAL_KEY_ENUMS: &'static [i32] = &[
    winuser::VK_LBUTTON,
    winuser::VK_RBUTTON,
    winuser::VK_CANCEL,
    winuser::VK_MBUTTON,
    winuser::VK_XBUTTON1,
    winuser::VK_XBUTTON2,
    winuser::VK_BACK,
    winuser::VK_TAB,
    winuser::VK_CLEAR,
    winuser::VK_RETURN,
    winuser::VK_SHIFT,
    winuser::VK_CONTROL,
    winuser::VK_MENU,
    winuser::VK_PAUSE,
    winuser::VK_CAPITAL,
    winuser::VK_KANA,
    winuser::VK_HANGEUL,
    winuser::VK_HANGUL,
    winuser::VK_JUNJA,
    winuser::VK_FINAL,
    winuser::VK_HANJA,
    winuser::VK_KANJI,
    winuser::VK_ESCAPE,
    winuser::VK_CONVERT,
    winuser::VK_NONCONVERT,
    winuser::VK_ACCEPT,
    winuser::VK_MODECHANGE,
    winuser::VK_SPACE,
    winuser::VK_PRIOR,
    winuser::VK_NEXT,
    winuser::VK_END,
    winuser::VK_HOME,
    winuser::VK_LEFT,
    winuser::VK_UP,
    winuser::VK_RIGHT,
    winuser::VK_DOWN,
    winuser::VK_SELECT,
    winuser::VK_PRINT,
    winuser::VK_EXECUTE,
    winuser::VK_SNAPSHOT,
    winuser::VK_INSERT,
    winuser::VK_DELETE,
    winuser::VK_HELP,
    winuser::VK_LWIN,
    winuser::VK_RWIN,
    winuser::VK_APPS,
    winuser::VK_SLEEP,
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
    winuser::VK_F1,
    winuser::VK_F2,
    winuser::VK_F3,
    winuser::VK_F4,
    winuser::VK_F5,
    winuser::VK_F6,
    winuser::VK_F7,
    winuser::VK_F8,
    winuser::VK_F9,
    winuser::VK_F10,
    winuser::VK_F11,
    winuser::VK_F12,
    winuser::VK_F13,
    winuser::VK_F14,
    winuser::VK_F15,
    winuser::VK_F16,
    winuser::VK_F17,
    winuser::VK_F18,
    winuser::VK_F19,
    winuser::VK_F20,
    winuser::VK_F21,
    winuser::VK_F22,
    winuser::VK_F23,
    winuser::VK_F24,
    winuser::VK_NAVIGATION_VIEW,
    winuser::VK_NAVIGATION_MENU,
    winuser::VK_NAVIGATION_UP,
    winuser::VK_NAVIGATION_DOWN,
    winuser::VK_NAVIGATION_LEFT,
    winuser::VK_NAVIGATION_RIGHT,
    winuser::VK_NAVIGATION_ACCEPT,
    winuser::VK_NAVIGATION_CANCEL,
    winuser::VK_NUMLOCK,
    winuser::VK_SCROLL,
    winuser::VK_OEM_NEC_EQUAL,
    winuser::VK_OEM_FJ_JISHO,
    winuser::VK_OEM_FJ_MASSHOU,
    winuser::VK_OEM_FJ_TOUROKU,
    winuser::VK_OEM_FJ_LOYA,
    winuser::VK_OEM_FJ_ROYA,
    winuser::VK_LSHIFT,
    winuser::VK_RSHIFT,
    winuser::VK_LCONTROL,
    winuser::VK_RCONTROL,
    winuser::VK_LMENU,
    winuser::VK_RMENU,
    winuser::VK_BROWSER_BACK,
    winuser::VK_BROWSER_FORWARD,
    winuser::VK_BROWSER_REFRESH,
    winuser::VK_BROWSER_STOP,
    winuser::VK_BROWSER_SEARCH,
    winuser::VK_BROWSER_FAVORITES,
    winuser::VK_BROWSER_HOME,
    winuser::VK_VOLUME_MUTE,
    winuser::VK_VOLUME_DOWN,
    winuser::VK_VOLUME_UP,
    winuser::VK_MEDIA_NEXT_TRACK,
    winuser::VK_MEDIA_PREV_TRACK,
    winuser::VK_MEDIA_STOP,
    winuser::VK_MEDIA_PLAY_PAUSE,
    winuser::VK_LAUNCH_MAIL,
    winuser::VK_LAUNCH_MEDIA_SELECT,
    winuser::VK_LAUNCH_APP1,
    winuser::VK_LAUNCH_APP2,
    winuser::VK_OEM_1,
    winuser::VK_OEM_PLUS,
    winuser::VK_OEM_COMMA,
    winuser::VK_OEM_MINUS,
    winuser::VK_OEM_PERIOD,
    winuser::VK_OEM_2,
    winuser::VK_OEM_3,
    winuser::VK_GAMEPAD_A,
    winuser::VK_GAMEPAD_B,
    winuser::VK_GAMEPAD_X,
    winuser::VK_GAMEPAD_Y,
    winuser::VK_GAMEPAD_RIGHT_SHOULDER,
    winuser::VK_GAMEPAD_LEFT_SHOULDER,
    winuser::VK_GAMEPAD_LEFT_TRIGGER,
    winuser::VK_GAMEPAD_RIGHT_TRIGGER,
    winuser::VK_GAMEPAD_DPAD_UP,
    winuser::VK_GAMEPAD_DPAD_DOWN,
    winuser::VK_GAMEPAD_DPAD_LEFT,
    winuser::VK_GAMEPAD_DPAD_RIGHT,
    winuser::VK_GAMEPAD_MENU,
    winuser::VK_GAMEPAD_VIEW,
    winuser::VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON,
    winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON,
    winuser::VK_GAMEPAD_LEFT_THUMBSTICK_UP,
    winuser::VK_GAMEPAD_LEFT_THUMBSTICK_DOWN,
    winuser::VK_GAMEPAD_LEFT_THUMBSTICK_RIGHT,
    winuser::VK_GAMEPAD_LEFT_THUMBSTICK_LEFT,
    winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_UP,
    winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_DOWN,
    winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_RIGHT,
    winuser::VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT,
    winuser::VK_OEM_4,
    winuser::VK_OEM_5,
    winuser::VK_OEM_6,
    winuser::VK_OEM_7,
    winuser::VK_OEM_8,
    winuser::VK_OEM_AX,
    winuser::VK_OEM_102,
    winuser::VK_ICO_HELP,
    winuser::VK_ICO_00,
    winuser::VK_PROCESSKEY,
    winuser::VK_ICO_CLEAR,
    winuser::VK_PACKET,
    winuser::VK_OEM_RESET,
    winuser::VK_OEM_JUMP,
    winuser::VK_OEM_PA1,
    winuser::VK_OEM_PA2,
    winuser::VK_OEM_PA3,
    winuser::VK_OEM_WSCTRL,
    winuser::VK_OEM_CUSEL,
    winuser::VK_OEM_ATTN,
    winuser::VK_OEM_FINISH,
    winuser::VK_OEM_COPY,
    winuser::VK_OEM_AUTO,
    winuser::VK_OEM_ENLW,
    winuser::VK_OEM_BACKTAB,
    winuser::VK_ATTN,
    winuser::VK_CRSEL,
    winuser::VK_EXSEL,
    winuser::VK_EREOF,
    winuser::VK_PLAY,
    winuser::VK_ZOOM,
    winuser::VK_NONAME,
    winuser::VK_PA1,
    winuser::VK_OEM_CLEAR,
];
