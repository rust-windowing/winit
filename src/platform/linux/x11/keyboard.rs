use libc::{c_int, c_uint};
use keyboard_types::*;
use super::ffi;

pub fn get_state(state: c_int) -> KeyState {
    match state {
        ffi::KeyPress => KeyState::Down,
        ffi::KeyRelease => KeyState::Up,
        _ => unreachable!()
    }
}

#[allow(non_upper_case_globals)]
pub fn get_key(keysym: c_uint, string: String) -> Key {
    use keyboard_types::Key::*;
    use x11_dl::keysym::*;
    match keysym {
        XK_Alt_L | XK_Alt_R => Alt,
        XK_ISO_Level3_Shift => AltGraph,
        XK_Caps_Lock => CapsLock,
        XK_Control_L | XK_Control_R => Control,
        XK_Super_L | XK_Super_R => Meta,
        XK_Num_Lock => NumLock,
        XK_Scroll_Lock => ScrollLock,
        XK_Shift_L | XK_Shift_R => Shift,
        XK_Return | XK_KP_Enter => Enter,
        XK_Tab => Tab,
        XK_Down | XK_KP_Down => ArrowDown,
        XK_Left | XK_KP_Left => ArrowLeft,
        XK_Right | XK_KP_Right => ArrowRight,
        XK_Up | XK_KP_Up => ArrowUp,
        XK_End | XK_KP_End => End,
        XK_Home | XK_KP_Home => Home,
        XK_Page_Down | XK_KP_Page_Down => PageDown,
        XK_Page_Up | XK_KP_Page_Up => PageUp,
        XK_BackSpace => Backspace,
        XK_Clear => Clear,
        XK_Delete => Delete,
        XK_Insert => Insert,
        XK_Redo => Redo,
        XK_Undo => Undo,
        XK_Escape => Escape,
        XF86XK_MonBrightnessDown => BrightnessDown,
        XF86XK_MonBrightnessUp => BrightnessUp,
        XK_Multi_key => Compose,
        XK_Hiragana => Hiragana,
        XK_Hiragana_Katakana => HiraganaKatakana,
        XK_Kanji => KanjiMode,
        XK_Katakana => Katakana,
        XK_Romaji => Romaji,
        XK_Zenkaku => Zenkaku,
        XK_Zenkaku_Hankaku => ZenkakuHankaku,
        XK_F1 => F1,
        XK_F2 => F2,
        XK_F3 => F3,
        XK_F4 => F4,
        XK_F5 => F5,
        XK_F6 => F6,
        XK_F7 => F7,
        XK_F8 => F8,
        XK_F9 => F9,
        XK_F10 => F10,
        XK_F11 => F11,
        XK_F12 => F12,
        XK_Print => Print,
        XF86XK_AudioLowerVolume => AudioVolumeDown,
        XF86XK_AudioRaiseVolume => AudioVolumeUp,
        XF86XK_AudioMute => AudioVolumeMute,

        XK_dead_A | XK_dead_E | XK_dead_I | XK_dead_O | XK_dead_U | XK_dead_a
        | XK_dead_abovecomma | XK_dead_abovedot | XK_dead_abovereversedcomma
        | XK_dead_abovering | XK_dead_aboveverticalline | XK_dead_acute
        | XK_dead_belowbreve | XK_dead_belowcircumflex | XK_dead_belowcomma
        | XK_dead_belowdiaeresis | XK_dead_belowdot | XK_dead_belowmacron
        | XK_dead_belowring | XK_dead_belowtilde | XK_dead_belowverticalline
        | XK_dead_breve | XK_dead_capital_schwa | XK_dead_caron
        | XK_dead_cedilla | XK_dead_circumflex | XK_dead_currency
        | XK_dead_diaeresis | XK_dead_doubleacute | XK_dead_doublegrave
        | XK_dead_e | XK_dead_grave | XK_dead_greek | XK_dead_hook
        | XK_dead_horn | XK_dead_i | XK_dead_invertedbreve | XK_dead_iota
        | XK_dead_longsolidusoverlay | XK_dead_lowline | XK_dead_macron
        | XK_dead_o | XK_dead_ogonek | XK_dead_semivoiced_sound
        | XK_dead_small_schwa | XK_dead_stroke | XK_dead_tilde | XK_dead_u
        | XK_dead_voiced_sound => Dead,
        _ if !string.is_empty() => Character(string),
        _ => Unidentified,
    }
}

#[allow(non_upper_case_globals)]
pub fn get_code(keycode: c_uint) -> Code {
    // See: https://www.w3.org/TR/uievents-code/
    use keyboard_types::Code::*;
    match keycode {
        9 => Escape,
        10 => Digit1,
        11 => Digit2,
        12 => Digit3,
        13 => Digit4,
        14 => Digit5,
        15 => Digit6,
        16 => Digit7,
        17 => Digit8,
        18 => Digit9,
        19 => Digit0,
        20 => Minus,
        21 => Equal,
        22 => Backspace,
        23 => Tab,
        24 => KeyQ,
        25 => KeyW,
        26 => KeyE,
        27 => KeyR,
        28 => KeyT,
        29 => KeyY,
        30 => KeyU,
        31 => KeyI,
        32 => KeyO,
        33 => KeyP,
        34 => BracketLeft,
        35 => BracketRight,
        36 => Enter,
        38 => KeyA,
        37 => ControlLeft,
        39 => KeyS,
        40 => KeyD,
        41 => KeyF,
        42 => KeyG,
        43 => KeyH,
        44 => KeyJ,
        45 => KeyK,
        46 => KeyL,
        47 => Semicolon,
        48 => Quote,
        49 => Backquote,
        50 => ShiftLeft,
        51 => Backslash,
        52 => KeyZ,
        53 => KeyX,
        54 => KeyC,
        55 => KeyV,
        56 => KeyB,
        57 => KeyN,
        58 => KeyM,
        59 => Comma,
        60 => Period,
        61 => Slash,
        62 => ShiftRight,
        63 => NumpadMultiply,
        64 => AltLeft,
        65 => Space,
        66 => CapsLock,
        67 => F1,
        68 => F2,
        69 => F3,
        70 => F4,
        71 => F5,
        72 => F6,
        73 => F7,
        74 => F8,
        75 => F9,
        76 => F10,
        79 => Numpad7,
        80 => Numpad8,
        81 => Numpad9,
        82 => NumpadSubtract,
        83 => Numpad4,
        84 => Numpad5,
        85 => Numpad6,
        86 => NumpadAdd,
        87 => Numpad1,
        88 => Numpad2,
        89 => Numpad3,
        90 => Numpad0,
        94 => IntlBackslash,
        95 => F11,
        96 => F12,
        104 => NumpadEnter,
        105 => ControlRight,
        106 => NumpadDivide,
        107 => PrintScreen,
        108 => AltRight,
        110 => Home,
        111 => ArrowUp,
        112 => PageUp,
        113 => ArrowLeft,
        114 => ArrowRight,
        115 => End,
        116 => ArrowDown,
        117 => PageDown,
        118 => Insert,
        119 => Delete,
        133 => MetaLeft,
        _ => Unidentified,
    }
}

#[allow(non_upper_case_globals)]
pub fn get_key_location(keysym: c_uint) -> Location {
    // See: https://www.w3.org/TR/uievents/#events-keyboard-key-location
    use keyboard_types::Location::*;
    use x11_dl::keysym::*;
    match keysym {
        XK_Shift_L | XK_Control_L | XK_Alt_L | XK_Super_L => Left,
        XK_Shift_R | XK_Control_R | XK_Alt_R | XK_Super_R => Right,
        XK_KP_Down | XK_KP_Left | XK_KP_Right | XK_KP_Up => Numpad,
        XK_KP_End | XK_KP_Home | XK_KP_Page_Down | XK_KP_Page_Up => Numpad,
        XK_KP_0 | XK_KP_1 | XK_KP_2 | XK_KP_3 | XK_KP_4 | XK_KP_5 | XK_KP_6
        | XK_KP_7 | XK_KP_8 | XK_KP_9 | XK_KP_Separator | XK_KP_Enter
        | XK_KP_Add | XK_KP_Subtract | XK_KP_Multiply | XK_KP_Divide => Numpad,
        _ => Standard,
    }
}

pub fn get_key_modifiers(state: c_uint) -> Modifiers {
    // See: https://stackoverflow.com/a/29001687/4255842
    let mut modifiers = Modifiers::empty();
    if state & ffi::ShiftMask != 0 { modifiers.insert(SHIFT) }
    if state & ffi::LockMask != 0 { modifiers.insert(CAPS_LOCK) }
    if state & ffi::ControlMask != 0 { modifiers.insert(CONTROL) }
    if state & ffi::Mod1Mask != 0 { modifiers.insert(ALT) }
    if state & ffi::Mod2Mask != 0 { modifiers.insert(NUM_LOCK) }
    if state & ffi::Mod3Mask != 0 { modifiers.insert(SCROLL_LOCK) }
    if state & ffi::Mod4Mask != 0 { modifiers.insert(META) }
    if state & ffi::Mod5Mask != 0 { modifiers.insert(ALT_GRAPH) }
    modifiers
}

#[allow(non_upper_case_globals)]
pub fn get_dead_key_combining_character(keysym: c_uint) -> Option<char> {
    use x11_dl::keysym::*;
    // See: https://www.cl.cam.ac.uk/~mgk25/ucs/keysyms.txt
    Some(match keysym {
        XK_dead_grave => '\u{0300}',
        XK_dead_acute => '\u{0301}',
        XK_dead_circumflex => '\u{0302}',
        XK_dead_tilde => '\u{0303}',
        XK_dead_macron => '\u{0304}',
        XK_dead_breve => '\u{0306}',
        XK_dead_abovedot => '\u{0307}',
        XK_dead_diaeresis => '\u{0308}',
        XK_dead_abovering => '\u{030a}',
        XK_dead_doubleacute => '\u{030b}',
        XK_dead_caron => '\u{030c}',
        XK_dead_cedilla => '\u{0327}',
        XK_dead_ogonek => '\u{0328}',
        XK_dead_iota => '\u{0345}',
        XK_dead_voiced_sound => '\u{3099}',
        XK_dead_semivoiced_sound => '\u{309a}',
        // TODO: Cover all diacritical marks.
        _ => return None,
    })
}