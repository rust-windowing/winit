//! XKB keymap.

use std::sync::Arc;

use kbvm::lookup::LookupTable;
use kbvm::{evdev, xkb, GroupIndex, Keycode, Keysym, ModifierMask};
#[cfg(x11_platform)]
use {kbvm::xkb::x11::KbvmX11Ext, x11rb::xcb_ffi::XCBConnection};
#[cfg(wayland_platform)]
use {memmap2::MmapOptions, std::os::unix::io::OwnedFd};

use crate::keyboard::{Key, KeyCode, KeyLocation, NamedKey, NativeKey, NativeKeyCode, PhysicalKey};

/// Map the linux scancode to Keycode.
pub fn scancode_to_physicalkey(scancode: u32) -> PhysicalKey {
    keycode_to_physicalkey(Keycode::from_evdev(scancode))
}

/// Map the keycode to a physical key via the evdev mapping.
pub fn keycode_to_physicalkey(keycode: Keycode) -> PhysicalKey {
    // If Winit programs end up being run on other Unix-likes, I can only hope they agree on what
    // the keycodes mean.
    //
    // The mapping here is heavily influenced by Firefox' source:
    // https://searchfox.org/mozilla-central/rev/c597e9c789ad36af84a0370d395be066b7dc94f4/widget/NativeKeyToDOMCodeName.h
    //
    // Some of the keycodes are likely superfluous for our purposes, and some are ones which are
    // difficult to test the correctness of, or discover the purpose of. Because of this, they've
    // either been commented out here, or not included at all.
    if keycode.to_evdev() == 0 {
        return PhysicalKey::Unidentified(NativeKeyCode::Xkb(0));
    }
    PhysicalKey::Code(match keycode {
        evdev::ESC => KeyCode::Escape,
        evdev::_1 => KeyCode::Digit1,
        evdev::_2 => KeyCode::Digit2,
        evdev::_3 => KeyCode::Digit3,
        evdev::_4 => KeyCode::Digit4,
        evdev::_5 => KeyCode::Digit5,
        evdev::_6 => KeyCode::Digit6,
        evdev::_7 => KeyCode::Digit7,
        evdev::_8 => KeyCode::Digit8,
        evdev::_9 => KeyCode::Digit9,
        evdev::_0 => KeyCode::Digit0,
        evdev::MINUS => KeyCode::Minus,
        evdev::EQUAL => KeyCode::Equal,
        evdev::BACKSPACE => KeyCode::Backspace,
        evdev::TAB => KeyCode::Tab,
        evdev::Q => KeyCode::KeyQ,
        evdev::W => KeyCode::KeyW,
        evdev::E => KeyCode::KeyE,
        evdev::R => KeyCode::KeyR,
        evdev::T => KeyCode::KeyT,
        evdev::Y => KeyCode::KeyY,
        evdev::U => KeyCode::KeyU,
        evdev::I => KeyCode::KeyI,
        evdev::O => KeyCode::KeyO,
        evdev::P => KeyCode::KeyP,
        evdev::LEFTBRACE => KeyCode::BracketLeft,
        evdev::RIGHTBRACE => KeyCode::BracketRight,
        evdev::ENTER => KeyCode::Enter,
        evdev::LEFTCTRL => KeyCode::ControlLeft,
        evdev::A => KeyCode::KeyA,
        evdev::S => KeyCode::KeyS,
        evdev::D => KeyCode::KeyD,
        evdev::F => KeyCode::KeyF,
        evdev::G => KeyCode::KeyG,
        evdev::H => KeyCode::KeyH,
        evdev::J => KeyCode::KeyJ,
        evdev::K => KeyCode::KeyK,
        evdev::L => KeyCode::KeyL,
        evdev::SEMICOLON => KeyCode::Semicolon,
        evdev::APOSTROPHE => KeyCode::Quote,
        evdev::GRAVE => KeyCode::Backquote,
        evdev::LEFTSHIFT => KeyCode::ShiftLeft,
        evdev::BACKSLASH => KeyCode::Backslash,
        evdev::Z => KeyCode::KeyZ,
        evdev::X => KeyCode::KeyX,
        evdev::C => KeyCode::KeyC,
        evdev::V => KeyCode::KeyV,
        evdev::B => KeyCode::KeyB,
        evdev::N => KeyCode::KeyN,
        evdev::M => KeyCode::KeyM,
        evdev::COMMA => KeyCode::Comma,
        evdev::DOT => KeyCode::Period,
        evdev::SLASH => KeyCode::Slash,
        evdev::RIGHTSHIFT => KeyCode::ShiftRight,
        evdev::KPASTERISK => KeyCode::NumpadMultiply,
        evdev::LEFTALT => KeyCode::AltLeft,
        evdev::SPACE => KeyCode::Space,
        evdev::CAPSLOCK => KeyCode::CapsLock,
        evdev::F1 => KeyCode::F1,
        evdev::F2 => KeyCode::F2,
        evdev::F3 => KeyCode::F3,
        evdev::F4 => KeyCode::F4,
        evdev::F5 => KeyCode::F5,
        evdev::F6 => KeyCode::F6,
        evdev::F7 => KeyCode::F7,
        evdev::F8 => KeyCode::F8,
        evdev::F9 => KeyCode::F9,
        evdev::F10 => KeyCode::F10,
        evdev::NUMLOCK => KeyCode::NumLock,
        evdev::SCROLLLOCK => KeyCode::ScrollLock,
        evdev::KP7 => KeyCode::Numpad7,
        evdev::KP8 => KeyCode::Numpad8,
        evdev::KP9 => KeyCode::Numpad9,
        evdev::KPMINUS => KeyCode::NumpadSubtract,
        evdev::KP4 => KeyCode::Numpad4,
        evdev::KP5 => KeyCode::Numpad5,
        evdev::KP6 => KeyCode::Numpad6,
        evdev::KPPLUS => KeyCode::NumpadAdd,
        evdev::KP1 => KeyCode::Numpad1,
        evdev::KP2 => KeyCode::Numpad2,
        evdev::KP3 => KeyCode::Numpad3,
        evdev::KP0 => KeyCode::Numpad0,
        evdev::KPDOT => KeyCode::NumpadDecimal,
        evdev::ZENKAKUHANKAKU => KeyCode::Lang5,
        evdev::_102ND => KeyCode::IntlBackslash,
        evdev::F11 => KeyCode::F11,
        evdev::F12 => KeyCode::F12,
        evdev::RO => KeyCode::IntlRo,
        evdev::KATAKANA => KeyCode::Lang3,
        evdev::HIRAGANA => KeyCode::Lang4,
        evdev::HENKAN => KeyCode::Convert,
        evdev::KATAKANAHIRAGANA => KeyCode::KanaMode,
        evdev::MUHENKAN => KeyCode::NonConvert,
        // evdev::KPJPCOMMA => KeyCode::KPJPCOMMA,
        evdev::KPENTER => KeyCode::NumpadEnter,
        evdev::RIGHTCTRL => KeyCode::ControlRight,
        evdev::KPSLASH => KeyCode::NumpadDivide,
        evdev::SYSRQ => KeyCode::PrintScreen,
        evdev::RIGHTALT => KeyCode::AltRight,
        // evdev::LINEFEED => KeyCode::LINEFEED,
        evdev::HOME => KeyCode::Home,
        evdev::UP => KeyCode::ArrowUp,
        evdev::PAGEUP => KeyCode::PageUp,
        evdev::LEFT => KeyCode::ArrowLeft,
        evdev::RIGHT => KeyCode::ArrowRight,
        evdev::END => KeyCode::End,
        evdev::DOWN => KeyCode::ArrowDown,
        evdev::PAGEDOWN => KeyCode::PageDown,
        evdev::INSERT => KeyCode::Insert,
        evdev::DELETE => KeyCode::Delete,
        // evdev::MACRO => KeyCode::MACRO,
        evdev::MUTE => KeyCode::AudioVolumeMute,
        evdev::VOLUMEDOWN => KeyCode::AudioVolumeDown,
        evdev::VOLUMEUP => KeyCode::AudioVolumeUp,
        // evdev::POWER => KeyCode::POWER,
        evdev::KPEQUAL => KeyCode::NumpadEqual,
        // evdev::KPPLUSMINUS => KeyCode::KPPLUSMINUS,
        evdev::PAUSE => KeyCode::Pause,
        // evdev::SCALE => KeyCode::SCALE,
        evdev::KPCOMMA => KeyCode::NumpadComma,
        evdev::HANGUEL => KeyCode::Lang1,
        evdev::HANJA => KeyCode::Lang2,
        evdev::YEN => KeyCode::IntlYen,
        evdev::LEFTMETA => KeyCode::SuperLeft,
        evdev::RIGHTMETA => KeyCode::SuperRight,
        evdev::COMPOSE => KeyCode::ContextMenu,
        evdev::STOP => KeyCode::BrowserStop,
        evdev::AGAIN => KeyCode::Again,
        evdev::PROPS => KeyCode::Props,
        evdev::UNDO => KeyCode::Undo,
        evdev::FRONT => KeyCode::Select, // FRONT
        evdev::COPY => KeyCode::Copy,
        evdev::OPEN => KeyCode::Open,
        evdev::PASTE => KeyCode::Paste,
        evdev::FIND => KeyCode::Find,
        evdev::CUT => KeyCode::Cut,
        evdev::HELP => KeyCode::Help,
        // evdev::MENU => KeyCode::MENU,
        evdev::CALC => KeyCode::LaunchApp2, // CALC
        // evdev::SETUP => KeyCode::SETUP,
        // evdev::SLEEP => KeyCode::SLEEP,
        evdev::WAKEUP => KeyCode::WakeUp,
        evdev::FILE => KeyCode::LaunchApp1, // FILE
        // evdev::SENDFILE => KeyCode::SENDFILE,
        // evdev::DELETEFILE => KeyCode::DELETEFILE,
        // evdev::XFER => KeyCode::XFER,
        // evdev::PROG1 => KeyCode::PROG1,
        // evdev::PROG2 => KeyCode::PROG2,
        // evdev::WWW => KeyCode::WWW,
        // evdev::MSDOS => KeyCode::MSDOS,
        // evdev::COFFEE => KeyCode::COFFEE,
        // evdev::ROTATE_DISPLAY => KeyCode::ROTATE_DISPLAY,
        // evdev::CYCLEWINDOWS => KeyCode::CYCLEWINDOWS,
        evdev::MAIL => KeyCode::LaunchMail,
        evdev::BOOKMARKS => KeyCode::BrowserFavorites, // BOOKMARKS
        // evdev::COMPUTER => KeyCode::COMPUTER,
        evdev::BACK => KeyCode::BrowserBack,
        evdev::FORWARD => KeyCode::BrowserForward,
        // evdev::CLOSECD => KeyCode::CLOSECD,
        evdev::EJECTCD => KeyCode::Eject, // EJECTCD
        // evdev::EJECTCLOSECD => KeyCode::EJECTCLOSECD,
        evdev::NEXTSONG => KeyCode::MediaTrackNext,
        evdev::PLAYPAUSE => KeyCode::MediaPlayPause,
        evdev::PREVIOUSSONG => KeyCode::MediaTrackPrevious,
        evdev::STOPCD => KeyCode::MediaStop,
        // evdev::RECORD => KeyCode::RECORD,
        // evdev::REWIND => KeyCode::REWIND,
        // evdev::PHONE => KeyCode::PHONE,
        // evdev::ISO => KeyCode::ISO,
        evdev::CONFIG => KeyCode::MediaSelect, // CONFIG
        evdev::HOMEPAGE => KeyCode::BrowserHome,
        evdev::REFRESH => KeyCode::BrowserRefresh,
        // evdev::EXIT => KeyCode::EXIT,
        // evdev::MOVE => KeyCode::MOVE,
        // evdev::EDIT => KeyCode::EDIT,
        // evdev::SCROLLUP => KeyCode::SCROLLUP,
        // evdev::SCROLLDOWN => KeyCode::SCROLLDOWN,
        // evdev::KPLEFTPAREN => KeyCode::KPLEFTPAREN,
        // evdev::KPRIGHTPAREN => KeyCode::KPRIGHTPAREN,
        // evdev::NEW => KeyCode::NEW,
        // evdev::REDO => KeyCode::REDO,
        evdev::F13 => KeyCode::F13,
        evdev::F14 => KeyCode::F14,
        evdev::F15 => KeyCode::F15,
        evdev::F16 => KeyCode::F16,
        evdev::F17 => KeyCode::F17,
        evdev::F18 => KeyCode::F18,
        evdev::F19 => KeyCode::F19,
        evdev::F20 => KeyCode::F20,
        evdev::F21 => KeyCode::F21,
        evdev::F22 => KeyCode::F22,
        evdev::F23 => KeyCode::F23,
        evdev::F24 => KeyCode::F24,
        // evdev::PLAYCD => KeyCode::PLAYCD,
        // evdev::PAUSECD => KeyCode::PAUSECD,
        // evdev::PROG3 => KeyCode::PROG3,
        // evdev::PROG4 => KeyCode::PROG4,
        // evdev::DASHBOARD => KeyCode::DASHBOARD,
        // evdev::SUSPEND => KeyCode::SUSPEND,
        // evdev::CLOSE => KeyCode::CLOSE,
        // evdev::PLAY => KeyCode::PLAY,
        // evdev::FASTFORWARD => KeyCode::FASTFORWARD,
        // evdev::BASSBOOST => KeyCode::BASSBOOST,
        // evdev::PRINT => KeyCode::PRINT,
        // evdev::HP => KeyCode::HP,
        // evdev::CAMERA => KeyCode::CAMERA,
        // evdev::SOUND => KeyCode::SOUND,
        // evdev::QUESTION => KeyCode::QUESTION,
        // evdev::EMAIL => KeyCode::EMAIL,
        // evdev::CHAT => KeyCode::CHAT,
        evdev::SEARCH => KeyCode::BrowserSearch,
        // evdev::CONNECT => KeyCode::CONNECT,
        // evdev::FINANCE => KeyCode::FINANCE,
        // evdev::SPORT => KeyCode::SPORT,
        // evdev::SHOP => KeyCode::SHOP,
        // evdev::ALTERASE => KeyCode::ALTERASE,
        // evdev::CANCEL => KeyCode::CANCEL,
        // evdev::BRIGHTNESSDOWN => KeyCode::BRIGHTNESSDOW,
        // evdev::BRIGHTNESSUP => KeyCode::BRIGHTNESSU,
        // evdev::MEDIA => KeyCode::MEDIA,
        // evdev::SWITCHVIDEOMODE => KeyCode::SWITCHVIDEOMODE,
        // evdev::KBDILLUMTOGGLE => KeyCode::KBDILLUMTOGGLE,
        // evdev::KBDILLUMDOWN => KeyCode::KBDILLUMDOWN,
        // evdev::KBDILLUMUP => KeyCode::KBDILLUMUP,
        // evdev::SEND => KeyCode::SEND,
        // evdev::REPLY => KeyCode::REPLY,
        // evdev::FORWARDMAIL => KeyCode::FORWARDMAIL,
        // evdev::SAVE => KeyCode::SAVE,
        // evdev::DOCUMENTS => KeyCode::DOCUMENTS,
        // evdev::BATTERY => KeyCode::BATTERY,
        // evdev::BLUETOOTH => KeyCode::BLUETOOTH,
        // evdev::WLAN => KeyCode::WLAN,
        // evdev::UWB => KeyCode::UWB,
        evdev::UNKNOWN => return PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
        // evdev::VIDEO_NEXT => KeyCode::VIDEO_NEXT,
        // evdev::VIDEO_PREV => KeyCode::VIDEO_PREV,
        // evdev::BRIGHTNESS_CYCLE => KeyCode::BRIGHTNESS_CYCLE,
        // evdev::BRIGHTNESS_AUTO => KeyCode::BRIGHTNESS_AUTO,
        // evdev::DISPLAY_OFF => KeyCode::DISPLAY_OFF,
        // evdev::WWAN => KeyCode::WWAN,
        // evdev::RFKILL => KeyCode::RFKILL,
        // evdev::MICMUTE => KeyCode::KEY_MICMUTE,
        _ => return PhysicalKey::Unidentified(NativeKeyCode::Xkb(keycode.to_evdev())),
    })
}

pub fn physicalkey_to_scancode(key: PhysicalKey) -> Option<u32> {
    let code = match key {
        PhysicalKey::Code(code) => code,
        PhysicalKey::Unidentified(code) => {
            return match code {
                NativeKeyCode::Unidentified => Some(240),
                NativeKeyCode::Xkb(raw) => Some(raw),
                _ => None,
            };
        },
    };

    let keycode = match code {
        KeyCode::Escape => evdev::ESC,
        KeyCode::Digit1 => evdev::_1,
        KeyCode::Digit2 => evdev::_2,
        KeyCode::Digit3 => evdev::_3,
        KeyCode::Digit4 => evdev::_4,
        KeyCode::Digit5 => evdev::_5,
        KeyCode::Digit6 => evdev::_6,
        KeyCode::Digit7 => evdev::_7,
        KeyCode::Digit8 => evdev::_8,
        KeyCode::Digit9 => evdev::_9,
        KeyCode::Digit0 => evdev::_0,
        KeyCode::Minus => evdev::MINUS,
        KeyCode::Equal => evdev::EQUAL,
        KeyCode::Backspace => evdev::BACKSPACE,
        KeyCode::Tab => evdev::TAB,
        KeyCode::KeyQ => evdev::Q,
        KeyCode::KeyW => evdev::W,
        KeyCode::KeyE => evdev::E,
        KeyCode::KeyR => evdev::R,
        KeyCode::KeyT => evdev::T,
        KeyCode::KeyY => evdev::Y,
        KeyCode::KeyU => evdev::U,
        KeyCode::KeyI => evdev::I,
        KeyCode::KeyO => evdev::O,
        KeyCode::KeyP => evdev::P,
        KeyCode::BracketLeft => evdev::LEFTBRACE,
        KeyCode::BracketRight => evdev::RIGHTBRACE,
        KeyCode::Enter => evdev::ENTER,
        KeyCode::ControlLeft => evdev::LEFTCTRL,
        KeyCode::KeyA => evdev::A,
        KeyCode::KeyS => evdev::S,
        KeyCode::KeyD => evdev::D,
        KeyCode::KeyF => evdev::F,
        KeyCode::KeyG => evdev::G,
        KeyCode::KeyH => evdev::H,
        KeyCode::KeyJ => evdev::J,
        KeyCode::KeyK => evdev::K,
        KeyCode::KeyL => evdev::L,
        KeyCode::Semicolon => evdev::SEMICOLON,
        KeyCode::Quote => evdev::APOSTROPHE,
        KeyCode::Backquote => evdev::GRAVE,
        KeyCode::ShiftLeft => evdev::LEFTSHIFT,
        KeyCode::Backslash => evdev::BACKSLASH,
        KeyCode::KeyZ => evdev::Z,
        KeyCode::KeyX => evdev::X,
        KeyCode::KeyC => evdev::C,
        KeyCode::KeyV => evdev::V,
        KeyCode::KeyB => evdev::B,
        KeyCode::KeyN => evdev::N,
        KeyCode::KeyM => evdev::M,
        KeyCode::Comma => evdev::COMMA,
        KeyCode::Period => evdev::DOT,
        KeyCode::Slash => evdev::SLASH,
        KeyCode::ShiftRight => evdev::RIGHTSHIFT,
        KeyCode::NumpadMultiply => evdev::KPASTERISK,
        KeyCode::AltLeft => evdev::LEFTALT,
        KeyCode::Space => evdev::SPACE,
        KeyCode::CapsLock => evdev::CAPSLOCK,
        KeyCode::F1 => evdev::F1,
        KeyCode::F2 => evdev::F2,
        KeyCode::F3 => evdev::F3,
        KeyCode::F4 => evdev::F4,
        KeyCode::F5 => evdev::F5,
        KeyCode::F6 => evdev::F6,
        KeyCode::F7 => evdev::F7,
        KeyCode::F8 => evdev::F8,
        KeyCode::F9 => evdev::F9,
        KeyCode::F10 => evdev::F10,
        KeyCode::NumLock => evdev::NUMLOCK,
        KeyCode::ScrollLock => evdev::SCROLLLOCK,
        KeyCode::Numpad7 => evdev::KP7,
        KeyCode::Numpad8 => evdev::KP8,
        KeyCode::Numpad9 => evdev::KP9,
        KeyCode::NumpadSubtract => evdev::KPMINUS,
        KeyCode::Numpad4 => evdev::KP4,
        KeyCode::Numpad5 => evdev::KP5,
        KeyCode::Numpad6 => evdev::KP6,
        KeyCode::NumpadAdd => evdev::KPPLUS,
        KeyCode::Numpad1 => evdev::KP1,
        KeyCode::Numpad2 => evdev::KP2,
        KeyCode::Numpad3 => evdev::KP3,
        KeyCode::Numpad0 => evdev::KP0,
        KeyCode::NumpadDecimal => evdev::KPDOT,
        KeyCode::Lang5 => evdev::ZENKAKUHANKAKU,
        KeyCode::IntlBackslash => evdev::_102ND,
        KeyCode::F11 => evdev::F11,
        KeyCode::F12 => evdev::F12,
        KeyCode::IntlRo => evdev::RO,
        KeyCode::Lang3 => evdev::KATAKANA,
        KeyCode::Lang4 => evdev::HIRAGANA,
        KeyCode::Convert => evdev::HENKAN,
        KeyCode::KanaMode => evdev::KATAKANAHIRAGANA,
        KeyCode::NonConvert => evdev::MUHENKAN,
        KeyCode::NumpadEnter => evdev::KPENTER,
        KeyCode::ControlRight => evdev::RIGHTCTRL,
        KeyCode::NumpadDivide => evdev::KPSLASH,
        KeyCode::PrintScreen => evdev::SYSRQ,
        KeyCode::AltRight => evdev::RIGHTALT,
        KeyCode::Home => evdev::HOME,
        KeyCode::ArrowUp => evdev::UP,
        KeyCode::PageUp => evdev::PAGEUP,
        KeyCode::ArrowLeft => evdev::LEFT,
        KeyCode::ArrowRight => evdev::RIGHT,
        KeyCode::End => evdev::END,
        KeyCode::ArrowDown => evdev::DOWN,
        KeyCode::PageDown => evdev::PAGEDOWN,
        KeyCode::Insert => evdev::INSERT,
        KeyCode::Delete => evdev::DELETE,
        KeyCode::AudioVolumeMute => evdev::MUTE,
        KeyCode::AudioVolumeDown => evdev::VOLUMEDOWN,
        KeyCode::AudioVolumeUp => evdev::VOLUMEUP,
        KeyCode::NumpadEqual => evdev::KPEQUAL,
        KeyCode::Pause => evdev::PAUSE,
        KeyCode::NumpadComma => evdev::KPCOMMA,
        KeyCode::Lang1 => evdev::HANGUEL,
        KeyCode::Lang2 => evdev::HANJA,
        KeyCode::IntlYen => evdev::YEN,
        KeyCode::SuperLeft => evdev::LEFTMETA,
        KeyCode::SuperRight => evdev::RIGHTMETA,
        KeyCode::ContextMenu => evdev::COMPOSE,
        KeyCode::BrowserStop => evdev::STOP,
        KeyCode::Again => evdev::AGAIN,
        KeyCode::Props => evdev::PROPS,
        KeyCode::Undo => evdev::UNDO,
        KeyCode::Select => evdev::FRONT,
        KeyCode::Copy => evdev::COPY,
        KeyCode::Open => evdev::OPEN,
        KeyCode::Paste => evdev::PASTE,
        KeyCode::Find => evdev::FIND,
        KeyCode::Cut => evdev::CUT,
        KeyCode::Help => evdev::HELP,
        KeyCode::LaunchApp2 => evdev::CALC,
        KeyCode::WakeUp => evdev::WAKEUP,
        KeyCode::LaunchApp1 => evdev::FILE,
        KeyCode::LaunchMail => evdev::MAIL,
        KeyCode::BrowserFavorites => evdev::BOOKMARKS,
        KeyCode::BrowserBack => evdev::BACK,
        KeyCode::BrowserForward => evdev::FORWARD,
        KeyCode::Eject => evdev::EJECTCD,
        KeyCode::MediaTrackNext => evdev::NEXTSONG,
        KeyCode::MediaPlayPause => evdev::PLAYPAUSE,
        KeyCode::MediaTrackPrevious => evdev::PREVIOUSSONG,
        KeyCode::MediaStop => evdev::STOPCD,
        KeyCode::MediaSelect => evdev::CONFIG,
        KeyCode::BrowserHome => evdev::HOMEPAGE,
        KeyCode::BrowserRefresh => evdev::REFRESH,
        KeyCode::F13 => evdev::F13,
        KeyCode::F14 => evdev::F14,
        KeyCode::F15 => evdev::F15,
        KeyCode::F16 => evdev::F16,
        KeyCode::F17 => evdev::F17,
        KeyCode::F18 => evdev::F18,
        KeyCode::F19 => evdev::F19,
        KeyCode::F20 => evdev::F20,
        KeyCode::F21 => evdev::F21,
        KeyCode::F22 => evdev::F22,
        KeyCode::F23 => evdev::F23,
        KeyCode::F24 => evdev::F24,
        KeyCode::BrowserSearch => evdev::SEARCH,
        _ => return None,
    };
    Some(keycode.to_evdev())
}

pub fn keysym_to_key(keysym: Keysym) -> Key {
    use kbvm::syms as keysyms;
    Key::Named(match keysym {
        // TTY function keys
        keysyms::BackSpace => NamedKey::Backspace,
        keysyms::Tab => NamedKey::Tab,
        // keysyms::Linefeed => NamedKey::Linefeed,
        keysyms::Clear => NamedKey::Clear,
        keysyms::Return => NamedKey::Enter,
        keysyms::Pause => NamedKey::Pause,
        keysyms::Scroll_Lock => NamedKey::ScrollLock,
        keysyms::Sys_Req => NamedKey::PrintScreen,
        keysyms::Escape => NamedKey::Escape,
        keysyms::Delete => NamedKey::Delete,

        // IME keys
        keysyms::Multi_key => NamedKey::Compose,
        keysyms::Codeinput => NamedKey::CodeInput,
        keysyms::SingleCandidate => NamedKey::SingleCandidate,
        keysyms::MultipleCandidate => NamedKey::AllCandidates,
        keysyms::PreviousCandidate => NamedKey::PreviousCandidate,

        // Japanese keys
        keysyms::Kanji => NamedKey::KanjiMode,
        keysyms::Muhenkan => NamedKey::NonConvert,
        keysyms::Henkan_Mode => NamedKey::Convert,
        keysyms::Romaji => NamedKey::Romaji,
        keysyms::Hiragana => NamedKey::Hiragana,
        keysyms::Hiragana_Katakana => NamedKey::HiraganaKatakana,
        keysyms::Zenkaku => NamedKey::Zenkaku,
        keysyms::Hankaku => NamedKey::Hankaku,
        keysyms::Zenkaku_Hankaku => NamedKey::ZenkakuHankaku,
        // keysyms::Touroku => NamedKey::Touroku,
        // keysyms::Massyo => NamedKey::Massyo,
        keysyms::Kana_Lock => NamedKey::KanaMode,
        keysyms::Kana_Shift => NamedKey::KanaMode,
        keysyms::Eisu_Shift => NamedKey::Alphanumeric,
        keysyms::Eisu_toggle => NamedKey::Alphanumeric,
        // NOTE: The next three items are aliases for values we've already mapped.
        // keysyms::Kanji_Bangou => NamedKey::CodeInput,
        // keysyms::Zen_Koho => NamedKey::AllCandidates,
        // keysyms::Mae_Koho => NamedKey::PreviousCandidate,

        // Cursor control & motion
        keysyms::Home => NamedKey::Home,
        keysyms::Left => NamedKey::ArrowLeft,
        keysyms::Up => NamedKey::ArrowUp,
        keysyms::Right => NamedKey::ArrowRight,
        keysyms::Down => NamedKey::ArrowDown,
        // keysyms::Prior => NamedKey::PageUp,
        keysyms::Page_Up => NamedKey::PageUp,
        // keysyms::Next => NamedKey::PageDown,
        keysyms::Page_Down => NamedKey::PageDown,
        keysyms::End => NamedKey::End,
        // keysyms::Begin => NamedKey::Begin,

        // Misc. functions
        keysyms::Select => NamedKey::Select,
        keysyms::Print => NamedKey::PrintScreen,
        keysyms::Execute => NamedKey::Execute,
        keysyms::Insert => NamedKey::Insert,
        keysyms::Undo => NamedKey::Undo,
        keysyms::Redo => NamedKey::Redo,
        keysyms::Menu => NamedKey::ContextMenu,
        keysyms::Find => NamedKey::Find,
        keysyms::Cancel => NamedKey::Cancel,
        keysyms::Help => NamedKey::Help,
        keysyms::Break => NamedKey::Pause,
        keysyms::Mode_switch => NamedKey::ModeChange,
        // keysyms::script_switch => NamedKey::ModeChange,
        keysyms::Num_Lock => NamedKey::NumLock,

        // Keypad keys
        // keysyms::KP_Space => return Key::Character(" "),
        keysyms::KP_Tab => NamedKey::Tab,
        keysyms::KP_Enter => NamedKey::Enter,
        keysyms::KP_F1 => NamedKey::F1,
        keysyms::KP_F2 => NamedKey::F2,
        keysyms::KP_F3 => NamedKey::F3,
        keysyms::KP_F4 => NamedKey::F4,
        keysyms::KP_Home => NamedKey::Home,
        keysyms::KP_Left => NamedKey::ArrowLeft,
        keysyms::KP_Up => NamedKey::ArrowUp,
        keysyms::KP_Right => NamedKey::ArrowRight,
        keysyms::KP_Down => NamedKey::ArrowDown,
        // keysyms::KP_Prior => NamedKey::PageUp,
        keysyms::KP_Page_Up => NamedKey::PageUp,
        // keysyms::KP_Next => NamedKey::PageDown,
        keysyms::KP_Page_Down => NamedKey::PageDown,
        keysyms::KP_End => NamedKey::End,
        // This is the key labeled "5" on the numpad when NumLock is off.
        // keysyms::KP_Begin => NamedKey::Begin,
        keysyms::KP_Insert => NamedKey::Insert,
        keysyms::KP_Delete => NamedKey::Delete,
        // keysyms::KP_Equal => NamedKey::Equal,
        // keysyms::KP_Multiply => NamedKey::Multiply,
        // keysyms::KP_Add => NamedKey::Add,
        // keysyms::KP_Separator => NamedKey::Separator,
        // keysyms::KP_Subtract => NamedKey::Subtract,
        // keysyms::KP_Decimal => NamedKey::Decimal,
        // keysyms::KP_Divide => NamedKey::Divide,

        // keysyms::KP_0 => return Key::Character("0"),
        // keysyms::KP_1 => return Key::Character("1"),
        // keysyms::KP_2 => return Key::Character("2"),
        // keysyms::KP_3 => return Key::Character("3"),
        // keysyms::KP_4 => return Key::Character("4"),
        // keysyms::KP_5 => return Key::Character("5"),
        // keysyms::KP_6 => return Key::Character("6"),
        // keysyms::KP_7 => return Key::Character("7"),
        // keysyms::KP_8 => return Key::Character("8"),
        // keysyms::KP_9 => return Key::Character("9"),

        // Function keys
        keysyms::F1 => NamedKey::F1,
        keysyms::F2 => NamedKey::F2,
        keysyms::F3 => NamedKey::F3,
        keysyms::F4 => NamedKey::F4,
        keysyms::F5 => NamedKey::F5,
        keysyms::F6 => NamedKey::F6,
        keysyms::F7 => NamedKey::F7,
        keysyms::F8 => NamedKey::F8,
        keysyms::F9 => NamedKey::F9,
        keysyms::F10 => NamedKey::F10,
        keysyms::F11 => NamedKey::F11,
        keysyms::F12 => NamedKey::F12,
        keysyms::F13 => NamedKey::F13,
        keysyms::F14 => NamedKey::F14,
        keysyms::F15 => NamedKey::F15,
        keysyms::F16 => NamedKey::F16,
        keysyms::F17 => NamedKey::F17,
        keysyms::F18 => NamedKey::F18,
        keysyms::F19 => NamedKey::F19,
        keysyms::F20 => NamedKey::F20,
        keysyms::F21 => NamedKey::F21,
        keysyms::F22 => NamedKey::F22,
        keysyms::F23 => NamedKey::F23,
        keysyms::F24 => NamedKey::F24,
        keysyms::F25 => NamedKey::F25,
        keysyms::F26 => NamedKey::F26,
        keysyms::F27 => NamedKey::F27,
        keysyms::F28 => NamedKey::F28,
        keysyms::F29 => NamedKey::F29,
        keysyms::F30 => NamedKey::F30,
        keysyms::F31 => NamedKey::F31,
        keysyms::F32 => NamedKey::F32,
        keysyms::F33 => NamedKey::F33,
        keysyms::F34 => NamedKey::F34,
        keysyms::F35 => NamedKey::F35,

        // Modifiers
        keysyms::Shift_L => NamedKey::Shift,
        keysyms::Shift_R => NamedKey::Shift,
        keysyms::Control_L => NamedKey::Control,
        keysyms::Control_R => NamedKey::Control,
        keysyms::Caps_Lock => NamedKey::CapsLock,
        // keysyms::Shift_Lock => NamedKey::ShiftLock,

        // keysyms::Meta_L => NamedKey::Meta,
        // keysyms::Meta_R => NamedKey::Meta,
        keysyms::Alt_L => NamedKey::Alt,
        keysyms::Alt_R => NamedKey::Alt,
        keysyms::Super_L => NamedKey::Super,
        keysyms::Super_R => NamedKey::Super,
        keysyms::Hyper_L => NamedKey::Hyper,
        keysyms::Hyper_R => NamedKey::Hyper,

        // XKB function and modifier keys
        // keysyms::ISO_Lock => NamedKey::IsoLock,
        // keysyms::ISO_Level2_Latch => NamedKey::IsoLevel2Latch,
        keysyms::ISO_Level3_Shift => NamedKey::AltGraph,
        keysyms::ISO_Level3_Latch => NamedKey::AltGraph,
        keysyms::ISO_Level3_Lock => NamedKey::AltGraph,
        // keysyms::ISO_Level5_Shift => NamedKey::IsoLevel5Shift,
        // keysyms::ISO_Level5_Latch => NamedKey::IsoLevel5Latch,
        // keysyms::ISO_Level5_Lock => NamedKey::IsoLevel5Lock,
        // keysyms::ISO_Group_Shift => NamedKey::IsoGroupShift,
        // keysyms::ISO_Group_Latch => NamedKey::IsoGroupLatch,
        // keysyms::ISO_Group_Lock => NamedKey::IsoGroupLock,
        keysyms::ISO_Next_Group => NamedKey::GroupNext,
        // keysyms::ISO_Next_Group_Lock => NamedKey::GroupNextLock,
        keysyms::ISO_Prev_Group => NamedKey::GroupPrevious,
        // keysyms::ISO_Prev_Group_Lock => NamedKey::GroupPreviousLock,
        keysyms::ISO_First_Group => NamedKey::GroupFirst,
        // keysyms::ISO_First_Group_Lock => NamedKey::GroupFirstLock,
        keysyms::ISO_Last_Group => NamedKey::GroupLast,
        // keysyms::ISO_Last_Group_Lock => NamedKey::GroupLastLock,
        keysyms::ISO_Left_Tab => NamedKey::Tab,
        // keysyms::ISO_Move_Line_Up => NamedKey::IsoMoveLineUp,
        // keysyms::ISO_Move_Line_Down => NamedKey::IsoMoveLineDown,
        // keysyms::ISO_Partial_Line_Up => NamedKey::IsoPartialLineUp,
        // keysyms::ISO_Partial_Line_Down => NamedKey::IsoPartialLineDown,
        // keysyms::ISO_Partial_Space_Left => NamedKey::IsoPartialSpaceLeft,
        // keysyms::ISO_Partial_Space_Right => NamedKey::IsoPartialSpaceRight,
        // keysyms::ISO_Set_Margin_Left => NamedKey::IsoSetMarginLeft,
        // keysyms::ISO_Set_Margin_Right => NamedKey::IsoSetMarginRight,
        // keysyms::ISO_Release_Margin_Left => NamedKey::IsoReleaseMarginLeft,
        // keysyms::ISO_Release_Margin_Right => NamedKey::IsoReleaseMarginRight,
        // keysyms::ISO_Release_Both_Margins => NamedKey::IsoReleaseBothMargins,
        // keysyms::ISO_Fast_Cursor_Left => NamedKey::IsoFastPointerLeft,
        // keysyms::ISO_Fast_Cursor_Right => NamedKey::IsoFastCursorRight,
        // keysyms::ISO_Fast_Cursor_Up => NamedKey::IsoFastCursorUp,
        // keysyms::ISO_Fast_Cursor_Down => NamedKey::IsoFastCursorDown,
        // keysyms::ISO_Continuous_Underline => NamedKey::IsoContinuousUnderline,
        // keysyms::ISO_Discontinuous_Underline => NamedKey::IsoDiscontinuousUnderline,
        // keysyms::ISO_Emphasize => NamedKey::IsoEmphasize,
        // keysyms::ISO_Center_Object => NamedKey::IsoCenterObject,
        keysyms::ISO_Enter => NamedKey::Enter,

        // dead_grave..dead_currency

        // dead_lowline..dead_longsolidusoverlay

        // dead_a..dead_capital_schwa

        // dead_greek

        // First_Virtual_Screen..Terminate_Server

        // AccessX_Enable..AudibleBell_Enable

        // Pointer_Left..Pointer_Drag5

        // Pointer_EnableKeys..Pointer_DfltBtnPrev

        // ch..C_H

        // 3270 terminal keys
        // keysyms::3270_Duplicate => NamedKey::Duplicate,
        // keysyms::3270_FieldMark => NamedKey::FieldMark,
        // keysyms::3270_Right2 => NamedKey::Right2,
        // keysyms::3270_Left2 => NamedKey::Left2,
        // keysyms::3270_BackTab => NamedKey::BackTab,
        keysyms::_3270_EraseEOF => NamedKey::EraseEof,
        // keysyms::3270_EraseInput => NamedKey::EraseInput,
        // keysyms::3270_Reset => NamedKey::Reset,
        // keysyms::3270_Quit => NamedKey::Quit,
        // keysyms::3270_PA1 => NamedKey::Pa1,
        // keysyms::3270_PA2 => NamedKey::Pa2,
        // keysyms::3270_PA3 => NamedKey::Pa3,
        // keysyms::3270_Test => NamedKey::Test,
        keysyms::_3270_Attn => NamedKey::Attn,
        // keysyms::3270_CursorBlink => NamedKey::CursorBlink,
        // keysyms::3270_AltCursor => NamedKey::AltCursor,
        // keysyms::3270_KeyClick => NamedKey::KeyClick,
        // keysyms::3270_Jump => NamedKey::Jump,
        // keysyms::3270_Ident => NamedKey::Ident,
        // keysyms::3270_Rule => NamedKey::Rule,
        // keysyms::3270_Copy => NamedKey::Copy,
        keysyms::_3270_Play => NamedKey::Play,
        // keysyms::3270_Setup => NamedKey::Setup,
        // keysyms::3270_Record => NamedKey::Record,
        // keysyms::3270_ChangeScreen => NamedKey::ChangeScreen,
        // keysyms::3270_DeleteWord => NamedKey::DeleteWord,
        keysyms::_3270_ExSelect => NamedKey::ExSel,
        keysyms::_3270_CursorSelect => NamedKey::CrSel,
        keysyms::_3270_PrintScreen => NamedKey::PrintScreen,
        keysyms::_3270_Enter => NamedKey::Enter,

        keysyms::space => NamedKey::Space,
        // exclam..Sinh_kunddaliya

        // XFree86
        // keysyms::XF86ModeLock => NamedKey::ModeLock,

        // XFree86 - Backlight controls
        keysyms::XF86MonBrightnessUp => NamedKey::BrightnessUp,
        keysyms::XF86MonBrightnessDown => NamedKey::BrightnessDown,
        // keysyms::XF86KbdLightOnOff => NamedKey::LightOnOff,
        // keysyms::XF86KbdBrightnessUp => NamedKey::KeyboardBrightnessUp,
        // keysyms::XF86KbdBrightnessDown => NamedKey::KeyboardBrightnessDown,

        // XFree86 - "Internet"
        keysyms::XF86Standby => NamedKey::Standby,
        keysyms::XF86AudioLowerVolume => NamedKey::AudioVolumeDown,
        keysyms::XF86AudioRaiseVolume => NamedKey::AudioVolumeUp,
        keysyms::XF86AudioPlay => NamedKey::MediaPlay,
        keysyms::XF86AudioStop => NamedKey::MediaStop,
        keysyms::XF86AudioPrev => NamedKey::MediaTrackPrevious,
        keysyms::XF86AudioNext => NamedKey::MediaTrackNext,
        keysyms::XF86HomePage => NamedKey::BrowserHome,
        keysyms::XF86Mail => NamedKey::LaunchMail,
        // keysyms::XF86Start => NamedKey::Start,
        keysyms::XF86Search => NamedKey::BrowserSearch,
        keysyms::XF86AudioRecord => NamedKey::MediaRecord,

        // XFree86 - PDA
        keysyms::XF86Calculator => NamedKey::LaunchApplication2,
        // keysyms::XF86Memo => NamedKey::Memo,
        // keysyms::XF86ToDoList => NamedKey::ToDoList,
        keysyms::XF86Calendar => NamedKey::LaunchCalendar,
        keysyms::XF86PowerDown => NamedKey::Power,
        // keysyms::XF86ContrastAdjust => NamedKey::AdjustContrast,
        // keysyms::XF86RockerUp => NamedKey::RockerUp,
        // keysyms::XF86RockerDown => NamedKey::RockerDown,
        // keysyms::XF86RockerEnter => NamedKey::RockerEnter,

        // XFree86 - More "Internet"
        keysyms::XF86Back => NamedKey::BrowserBack,
        keysyms::XF86Forward => NamedKey::BrowserForward,
        // keysyms::XF86Stop => NamedKey::Stop,
        keysyms::XF86Refresh => NamedKey::BrowserRefresh,
        keysyms::XF86PowerOff => NamedKey::Power,
        keysyms::XF86WakeUp => NamedKey::WakeUp,
        keysyms::XF86Eject => NamedKey::Eject,
        keysyms::XF86ScreenSaver => NamedKey::LaunchScreenSaver,
        keysyms::XF86WWW => NamedKey::LaunchWebBrowser,
        keysyms::XF86Sleep => NamedKey::Standby,
        keysyms::XF86Favorites => NamedKey::BrowserFavorites,
        keysyms::XF86AudioPause => NamedKey::MediaPause,
        // keysyms::XF86AudioMedia => NamedKey::AudioMedia,
        keysyms::XF86MyComputer => NamedKey::LaunchApplication1,
        // keysyms::XF86VendorHome => NamedKey::VendorHome,
        // keysyms::XF86LightBulb => NamedKey::LightBulb,
        // keysyms::XF86Shop => NamedKey::BrowserShop,
        // keysyms::XF86History => NamedKey::BrowserHistory,
        // keysyms::XF86OpenURL => NamedKey::OpenUrl,
        // keysyms::XF86AddFavorite => NamedKey::AddFavorite,
        // keysyms::XF86HotLinks => NamedKey::HotLinks,
        // keysyms::XF86BrightnessAdjust => NamedKey::BrightnessAdjust,
        // keysyms::XF86Finance => NamedKey::BrowserFinance,
        // keysyms::XF86Community => NamedKey::BrowserCommunity,
        keysyms::XF86AudioRewind => NamedKey::MediaRewind,
        // keysyms::XF86BackForward => Key::???,
        // XF86Launch0..XF86LaunchF

        // XF86ApplicationLeft..XF86CD
        keysyms::XF86Calculater => NamedKey::LaunchApplication2, // Nice typo, libxkbcommon :)
        // XF86Clear
        keysyms::XF86Close => NamedKey::Close,
        keysyms::XF86Copy => NamedKey::Copy,
        keysyms::XF86Cut => NamedKey::Cut,
        // XF86Display..XF86Documents
        keysyms::XF86Excel => NamedKey::LaunchSpreadsheet,
        // XF86Explorer..XF86iTouch
        keysyms::XF86LogOff => NamedKey::LogOff,
        // XF86Market..XF86MenuPB
        keysyms::XF86MySites => NamedKey::BrowserFavorites,
        keysyms::XF86New => NamedKey::New,
        // XF86News..XF86OfficeHome
        keysyms::XF86Open => NamedKey::Open,
        // XF86Option
        keysyms::XF86Paste => NamedKey::Paste,
        keysyms::XF86Phone => NamedKey::LaunchPhone,
        // XF86Q
        keysyms::XF86Reply => NamedKey::MailReply,
        keysyms::XF86Reload => NamedKey::BrowserRefresh,
        // XF86RotateWindows..XF86RotationKB
        keysyms::XF86Save => NamedKey::Save,
        // XF86ScrollUp..XF86ScrollClick
        keysyms::XF86Send => NamedKey::MailSend,
        keysyms::XF86Spell => NamedKey::SpellCheck,
        keysyms::XF86SplitScreen => NamedKey::SplitScreenToggle,
        // XF86Support..XF86User2KB
        keysyms::XF86Video => NamedKey::LaunchMediaPlayer,
        // XF86WheelButton
        keysyms::XF86Word => NamedKey::LaunchWordProcessor,
        // XF86Xfer
        keysyms::XF86ZoomIn => NamedKey::ZoomIn,
        keysyms::XF86ZoomOut => NamedKey::ZoomOut,

        // XF86Away..XF86Messenger
        keysyms::XF86WebCam => NamedKey::LaunchWebCam,
        keysyms::XF86MailForward => NamedKey::MailForward,
        // XF86Pictures
        keysyms::XF86Music => NamedKey::LaunchMusicPlayer,

        // XF86Battery..XF86UWB
        keysyms::XF86AudioForward => NamedKey::MediaFastForward,
        // XF86AudioRepeat
        keysyms::XF86AudioRandomPlay => NamedKey::RandomToggle,
        keysyms::XF86Subtitle => NamedKey::Subtitle,
        keysyms::XF86AudioCycleTrack => NamedKey::MediaAudioTrack,
        // XF86CycleAngle..XF86Blue
        keysyms::XF86Suspend => NamedKey::Standby,
        keysyms::XF86Hibernate => NamedKey::Hibernate,
        // XF86TouchpadToggle..XF86TouchpadOff
        keysyms::XF86AudioMute => NamedKey::AudioVolumeMute,

        // XF86Switch_VT_1..XF86Switch_VT_12

        // XF86Ungrab..XF86ClearGrab
        keysyms::XF86Next_VMode => NamedKey::VideoModeNext,
        // keysyms::XF86Prev_VMode => NamedKey::VideoModePrevious,
        // XF86LogWindowTree..XF86LogGrabInfo

        // SunFA_Grave..SunFA_Cedilla

        // keysyms::SunF36 => NamedKey::F36 | NamedKey::F11,
        // keysyms::SunF37 => NamedKey::F37 | NamedKey::F12,

        // keysyms::SunSys_Req => NamedKey::PrintScreen,
        // The next couple of xkb (until SunStop) are already handled.
        // SunPrint_Screen..SunPageDown

        // SunUndo..SunFront
        keysyms::SunCopy => NamedKey::Copy,
        keysyms::SunOpen => NamedKey::Open,
        keysyms::SunPaste => NamedKey::Paste,
        keysyms::SunCut => NamedKey::Cut,

        // SunPowerSwitch
        keysyms::SunAudioLowerVolume => NamedKey::AudioVolumeDown,
        keysyms::SunAudioMute => NamedKey::AudioVolumeMute,
        keysyms::SunAudioRaiseVolume => NamedKey::AudioVolumeUp,
        // SunVideoDegauss
        keysyms::SunVideoLowerBrightness => NamedKey::BrightnessDown,
        keysyms::SunVideoRaiseBrightness => NamedKey::BrightnessUp,
        // SunPowerSwitchShift
        Keysym(0) => return Key::Unidentified(NativeKey::Unidentified),
        _ => return Key::Unidentified(NativeKey::Xkb(keysym.0)),
    })
}

pub fn keysym_location(keysym: Keysym) -> KeyLocation {
    use kbvm::syms as keysyms;
    match keysym {
        keysyms::Shift_L
        | keysyms::Control_L
        | keysyms::Meta_L
        | keysyms::Alt_L
        | keysyms::Super_L
        | keysyms::Hyper_L => KeyLocation::Left,
        keysyms::Shift_R
        | keysyms::Control_R
        | keysyms::Meta_R
        | keysyms::Alt_R
        | keysyms::Super_R
        | keysyms::Hyper_R => KeyLocation::Right,
        keysyms::KP_0
        | keysyms::KP_1
        | keysyms::KP_2
        | keysyms::KP_3
        | keysyms::KP_4
        | keysyms::KP_5
        | keysyms::KP_6
        | keysyms::KP_7
        | keysyms::KP_8
        | keysyms::KP_9
        | keysyms::KP_Space
        | keysyms::KP_Tab
        | keysyms::KP_Enter
        | keysyms::KP_F1
        | keysyms::KP_F2
        | keysyms::KP_F3
        | keysyms::KP_F4
        | keysyms::KP_Home
        | keysyms::KP_Left
        | keysyms::KP_Up
        | keysyms::KP_Right
        | keysyms::KP_Down
        | keysyms::KP_Page_Up
        | keysyms::KP_Page_Down
        | keysyms::KP_End
        | keysyms::KP_Begin
        | keysyms::KP_Insert
        | keysyms::KP_Delete
        | keysyms::KP_Equal
        | keysyms::KP_Multiply
        | keysyms::KP_Add
        | keysyms::KP_Separator
        | keysyms::KP_Subtract
        | keysyms::KP_Decimal
        | keysyms::KP_Divide => KeyLocation::Numpad,
        _ => KeyLocation::Standard,
    }
}

#[derive(Debug)]
pub struct XkbKeymap {
    pub(super) keymap: Arc<LookupTable>,
    pub _core_keyboard_id: u16,
}

impl XkbKeymap {
    #[cfg(wayland_platform)]
    pub fn from_fd(context: &xkb::Context, fd: OwnedFd, size: usize) -> Option<Self> {
        use kbvm::xkb::diagnostic::WriteToLog;
        let map = unsafe { MmapOptions::new().len(size).map_copy_read_only(&fd).ok()? };
        let keymap = context.keymap_from_bytes(WriteToLog, None, &*map).ok()?;
        Some(Self::new_inner(keymap, 0))
    }

    #[cfg(x11_platform)]
    pub fn from_x11_keymap(xcb: &XCBConnection, core_keyboard_id: u16) -> Option<Self> {
        let keymap = xcb.get_xkb_keymap(core_keyboard_id).ok()?;
        Some(Self::new_inner(keymap, core_keyboard_id))
    }

    fn new_inner(keymap: xkb::Keymap, _core_keyboard_id: u16) -> Self {
        let keymap = Arc::new(keymap.to_builder().build_lookup_table());
        Self { keymap, _core_keyboard_id }
    }

    pub fn first_keysym_by_level(&mut self, layout: u32, keycode: Keycode) -> Keysym {
        self.keymap
            .lookup(GroupIndex(layout), ModifierMask::NONE, keycode)
            .into_iter()
            .next()
            .map(|p| p.keysym())
            .unwrap_or_default()
    }

    /// Check whether the given key repeats.
    pub fn key_repeats(&mut self, keycode: Keycode) -> bool {
        self.keymap.repeats(keycode)
    }
}
