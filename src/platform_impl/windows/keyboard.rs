use std::os::raw::c_int;

use winapi::{shared::minwindef::LPARAM, um::winuser};

use crate::{
    event::{KeyEvent, ScanCode},
    platform_impl::platform::event::KeyEventExtra,
};

#[derive(Debug, Copy, Clone)]
pub struct KeyLParam {
    pub scancode: u8,
    pub extended: bool,

    /// This is `previous_state XOR transition_state` see the lParam for WM_KEYDOWN and WM_KEYUP.
    pub is_repeat: bool,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct PlatformScanCode(pub u16);

impl PlatformScanCode {
    pub fn new(scancode: u8, extended: bool) -> PlatformScanCode {
        let ex_scancode = (scancode as u16) | (if extended { 0xE000 } else { 0 });
        PlatformScanCode(ex_scancode)
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

pub fn build_key_event(vkey: i32, lparam: KeyLParam, state: keyboard_types::KeyState) -> KeyEvent {
    let scancode = PlatformScanCode::new(lparam.scancode, lparam.extended);

    let physical_key = native_key_to_code(scancode);

    KeyEvent {
        scancode: ScanCode(scancode),
        location: get_location(vkey, lparam.extended, physical_key),
        physical_key,
        logical_key: keyboard_types::Key::Unidentified,
        state,
        repeat: lparam.is_repeat,
        platform_specific: KeyEventExtra {
            char_with_all_modifers: None,
            key_without_modifers: keyboard_types::Key::Unidentified,
        },
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
    code: keyboard_types::Code,
) -> keyboard_types::Location {
    use keyboard_types::{Code, Location};
    use winuser::*;
    const VK_ABNT_C2: c_int = 0xc2;

    // Use the native VKEY and the extended flag to cover most cases
    // This is taken from the `druid` software within
    // druid-shell/src/platform/windows/keyboard.rs
    match vkey {
        VK_LSHIFT | VK_LCONTROL | VK_LMENU | VK_LWIN => return Location::Left,
        VK_RSHIFT | VK_RCONTROL | VK_RMENU | VK_RWIN => return Location::Right,
        VK_RETURN if extended => return Location::Numpad,
        VK_INSERT | VK_DELETE | VK_END | VK_DOWN | VK_NEXT | VK_LEFT | VK_CLEAR | VK_RIGHT
        | VK_HOME | VK_UP | VK_PRIOR => {
            if extended {
                return Location::Standard;
            } else {
                return Location::Numpad;
            }
        }
        VK_NUMPAD0 | VK_NUMPAD1 | VK_NUMPAD2 | VK_NUMPAD3 | VK_NUMPAD4 | VK_NUMPAD5
        | VK_NUMPAD6 | VK_NUMPAD7 | VK_NUMPAD8 | VK_NUMPAD9 | VK_DECIMAL | VK_DIVIDE
        | VK_MULTIPLY | VK_SUBTRACT | VK_ADD | VK_ABNT_C2 => return Location::Numpad,
        _ => (),
    }

    match code {
        Code::NumpadAdd => Location::Numpad,
        Code::NumpadSubtract => Location::Numpad,
        Code::NumpadMultiply => Location::Numpad,
        Code::NumpadDivide => Location::Numpad,
        Code::NumpadComma => Location::Numpad,
        Code::NumpadDecimal => Location::Numpad,
        _ => Location::Standard,
    }
}
