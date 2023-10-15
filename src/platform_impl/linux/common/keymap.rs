//! Convert XKB keys to Winit keys.

use crate::keyboard::{Action, Key, KeyCode, KeyLocation, NativeKey, NativeKeyCode, PhysicalKey};

/// Map the raw X11-style keycode to the `KeyCode` enum.
///
/// X11-style keycodes are offset by 8 from the keycodes the Linux kernel uses.
pub fn raw_keycode_to_physicalkey(keycode: u32) -> PhysicalKey {
    scancode_to_keycode(keycode.saturating_sub(8))
}

/// Map the linux scancode to Keycode.
///
/// Both X11 and Wayland use keys with `+ 8` offset to linux scancode.
pub fn scancode_to_keycode(scancode: u32) -> PhysicalKey {
    // The keycode values are taken from linux/include/uapi/linux/input-event-codes.h, as
    // libxkbcommon's documentation seems to suggest that the keycode values we're interested in
    // are defined by the Linux kernel. If Winit programs end up being run on other Unix-likes,
    // I can only hope they agree on what the keycodes mean.
    //
    // Some of the keycodes are likely superfluous for our purposes, and some are ones which are
    // difficult to test the correctness of, or discover the purpose of. Because of this, they've
    // either been commented out here, or not included at all.
    PhysicalKey::Code(match scancode {
        0 => return PhysicalKey::Unidentified(NativeKeyCode::Xkb(0)),
        1 => KeyCode::Escape,
        2 => KeyCode::Digit1,
        3 => KeyCode::Digit2,
        4 => KeyCode::Digit3,
        5 => KeyCode::Digit4,
        6 => KeyCode::Digit5,
        7 => KeyCode::Digit6,
        8 => KeyCode::Digit7,
        9 => KeyCode::Digit8,
        10 => KeyCode::Digit9,
        11 => KeyCode::Digit0,
        12 => KeyCode::Minus,
        13 => KeyCode::Equal,
        14 => KeyCode::Backspace,
        15 => KeyCode::Tab,
        16 => KeyCode::KeyQ,
        17 => KeyCode::KeyW,
        18 => KeyCode::KeyE,
        19 => KeyCode::KeyR,
        20 => KeyCode::KeyT,
        21 => KeyCode::KeyY,
        22 => KeyCode::KeyU,
        23 => KeyCode::KeyI,
        24 => KeyCode::KeyO,
        25 => KeyCode::KeyP,
        26 => KeyCode::BracketLeft,
        27 => KeyCode::BracketRight,
        28 => KeyCode::Enter,
        29 => KeyCode::ControlLeft,
        30 => KeyCode::KeyA,
        31 => KeyCode::KeyS,
        32 => KeyCode::KeyD,
        33 => KeyCode::KeyF,
        34 => KeyCode::KeyG,
        35 => KeyCode::KeyH,
        36 => KeyCode::KeyJ,
        37 => KeyCode::KeyK,
        38 => KeyCode::KeyL,
        39 => KeyCode::Semicolon,
        40 => KeyCode::Quote,
        41 => KeyCode::Backquote,
        42 => KeyCode::ShiftLeft,
        43 => KeyCode::Backslash,
        44 => KeyCode::KeyZ,
        45 => KeyCode::KeyX,
        46 => KeyCode::KeyC,
        47 => KeyCode::KeyV,
        48 => KeyCode::KeyB,
        49 => KeyCode::KeyN,
        50 => KeyCode::KeyM,
        51 => KeyCode::Comma,
        52 => KeyCode::Period,
        53 => KeyCode::Slash,
        54 => KeyCode::ShiftRight,
        55 => KeyCode::NumpadMultiply,
        56 => KeyCode::AltLeft,
        57 => KeyCode::Space,
        58 => KeyCode::CapsLock,
        59 => KeyCode::F1,
        60 => KeyCode::F2,
        61 => KeyCode::F3,
        62 => KeyCode::F4,
        63 => KeyCode::F5,
        64 => KeyCode::F6,
        65 => KeyCode::F7,
        66 => KeyCode::F8,
        67 => KeyCode::F9,
        68 => KeyCode::F10,
        69 => KeyCode::NumLock,
        70 => KeyCode::ScrollLock,
        71 => KeyCode::Numpad7,
        72 => KeyCode::Numpad8,
        73 => KeyCode::Numpad9,
        74 => KeyCode::NumpadSubtract,
        75 => KeyCode::Numpad4,
        76 => KeyCode::Numpad5,
        77 => KeyCode::Numpad6,
        78 => KeyCode::NumpadAdd,
        79 => KeyCode::Numpad1,
        80 => KeyCode::Numpad2,
        81 => KeyCode::Numpad3,
        82 => KeyCode::Numpad0,
        83 => KeyCode::NumpadDecimal,
        85 => KeyCode::Lang5,
        86 => KeyCode::IntlBackslash,
        87 => KeyCode::F11,
        88 => KeyCode::F12,
        89 => KeyCode::IntlRo,
        90 => KeyCode::Lang3,
        91 => KeyCode::Lang4,
        92 => KeyCode::Convert,
        93 => KeyCode::KanaMode,
        94 => KeyCode::NonConvert,
        // 95 => KeyCode::KPJPCOMMA,
        96 => KeyCode::NumpadEnter,
        97 => KeyCode::ControlRight,
        98 => KeyCode::NumpadDivide,
        99 => KeyCode::PrintScreen,
        100 => KeyCode::AltRight,
        // 101 => KeyCode::LINEFEED,
        102 => KeyCode::Home,
        103 => KeyCode::ArrowUp,
        104 => KeyCode::PageUp,
        105 => KeyCode::ArrowLeft,
        106 => KeyCode::ArrowRight,
        107 => KeyCode::End,
        108 => KeyCode::ArrowDown,
        109 => KeyCode::PageDown,
        110 => KeyCode::Insert,
        111 => KeyCode::Delete,
        // 112 => KeyCode::MACRO,
        113 => KeyCode::AudioVolumeMute,
        114 => KeyCode::AudioVolumeDown,
        115 => KeyCode::AudioVolumeUp,
        // 116 => KeyCode::POWER,
        117 => KeyCode::NumpadEqual,
        // 118 => KeyCode::KPPLUSMINUS,
        119 => KeyCode::Pause,
        // 120 => KeyCode::SCALE,
        121 => KeyCode::NumpadComma,
        122 => KeyCode::Lang1,
        123 => KeyCode::Lang2,
        124 => KeyCode::IntlYen,
        125 => KeyCode::SuperLeft,
        126 => KeyCode::SuperRight,
        127 => KeyCode::ContextMenu,
        // 128 => KeyCode::STOP,
        // 129 => KeyCode::AGAIN,
        // 130 => KeyCode::PROPS,
        // 131 => KeyCode::UNDO,
        // 132 => KeyCode::FRONT,
        // 133 => KeyCode::COPY,
        // 134 => KeyCode::OPEN,
        // 135 => KeyCode::PASTE,
        // 136 => KeyCode::FIND,
        // 137 => KeyCode::CUT,
        // 138 => KeyCode::HELP,
        // 139 => KeyCode::MENU,
        // 140 => KeyCode::CALC,
        // 141 => KeyCode::SETUP,
        // 142 => KeyCode::SLEEP,
        // 143 => KeyCode::WAKEUP,
        // 144 => KeyCode::FILE,
        // 145 => KeyCode::SENDFILE,
        // 146 => KeyCode::DELETEFILE,
        // 147 => KeyCode::XFER,
        // 148 => KeyCode::PROG1,
        // 149 => KeyCode::PROG2,
        // 150 => KeyCode::WWW,
        // 151 => KeyCode::MSDOS,
        // 152 => KeyCode::COFFEE,
        // 153 => KeyCode::ROTATE_DISPLAY,
        // 154 => KeyCode::CYCLEWINDOWS,
        // 155 => KeyCode::MAIL,
        // 156 => KeyCode::BOOKMARKS,
        // 157 => KeyCode::COMPUTER,
        // 158 => KeyCode::BACK,
        // 159 => KeyCode::FORWARD,
        // 160 => KeyCode::CLOSECD,
        // 161 => KeyCode::EJECTCD,
        // 162 => KeyCode::EJECTCLOSECD,
        163 => KeyCode::MediaTrackNext,
        164 => KeyCode::MediaPlayPause,
        165 => KeyCode::MediaTrackPrevious,
        166 => KeyCode::MediaStop,
        // 167 => KeyCode::RECORD,
        // 168 => KeyCode::REWIND,
        // 169 => KeyCode::PHONE,
        // 170 => KeyCode::ISO,
        // 171 => KeyCode::CONFIG,
        // 172 => KeyCode::HOMEPAGE,
        // 173 => KeyCode::REFRESH,
        // 174 => KeyCode::EXIT,
        // 175 => KeyCode::MOVE,
        // 176 => KeyCode::EDIT,
        // 177 => KeyCode::SCROLLUP,
        // 178 => KeyCode::SCROLLDOWN,
        // 179 => KeyCode::KPLEFTPAREN,
        // 180 => KeyCode::KPRIGHTPAREN,
        // 181 => KeyCode::NEW,
        // 182 => KeyCode::REDO,
        183 => KeyCode::F13,
        184 => KeyCode::F14,
        185 => KeyCode::F15,
        186 => KeyCode::F16,
        187 => KeyCode::F17,
        188 => KeyCode::F18,
        189 => KeyCode::F19,
        190 => KeyCode::F20,
        191 => KeyCode::F21,
        192 => KeyCode::F22,
        193 => KeyCode::F23,
        194 => KeyCode::F24,
        // 200 => KeyCode::PLAYCD,
        // 201 => KeyCode::PAUSECD,
        // 202 => KeyCode::PROG3,
        // 203 => KeyCode::PROG4,
        // 204 => KeyCode::DASHBOARD,
        // 205 => KeyCode::SUSPEND,
        // 206 => KeyCode::CLOSE,
        // 207 => KeyCode::PLAY,
        // 208 => KeyCode::FASTFORWARD,
        // 209 => KeyCode::BASSBOOST,
        // 210 => KeyCode::PRINT,
        // 211 => KeyCode::HP,
        // 212 => KeyCode::CAMERA,
        // 213 => KeyCode::SOUND,
        // 214 => KeyCode::QUESTION,
        // 215 => KeyCode::EMAIL,
        // 216 => KeyCode::CHAT,
        // 217 => KeyCode::SEARCH,
        // 218 => KeyCode::CONNECT,
        // 219 => KeyCode::FINANCE,
        // 220 => KeyCode::SPORT,
        // 221 => KeyCode::SHOP,
        // 222 => KeyCode::ALTERASE,
        // 223 => KeyCode::CANCEL,
        // 224 => KeyCode::BRIGHTNESSDOW,
        // 225 => KeyCode::BRIGHTNESSU,
        // 226 => KeyCode::MEDIA,
        // 227 => KeyCode::SWITCHVIDEOMODE,
        // 228 => KeyCode::KBDILLUMTOGGLE,
        // 229 => KeyCode::KBDILLUMDOWN,
        // 230 => KeyCode::KBDILLUMUP,
        // 231 => KeyCode::SEND,
        // 232 => KeyCode::REPLY,
        // 233 => KeyCode::FORWARDMAIL,
        // 234 => KeyCode::SAVE,
        // 235 => KeyCode::DOCUMENTS,
        // 236 => KeyCode::BATTERY,
        // 237 => KeyCode::BLUETOOTH,
        // 238 => KeyCode::WLAN,
        // 239 => KeyCode::UWB,
        240 => return PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
        // 241 => KeyCode::VIDEO_NEXT,
        // 242 => KeyCode::VIDEO_PREV,
        // 243 => KeyCode::BRIGHTNESS_CYCLE,
        // 244 => KeyCode::BRIGHTNESS_AUTO,
        // 245 => KeyCode::DISPLAY_OFF,
        // 246 => KeyCode::WWAN,
        // 247 => KeyCode::RFKILL,
        // 248 => KeyCode::KEY_MICMUTE,
        _ => return PhysicalKey::Unidentified(NativeKeyCode::Xkb(scancode)),
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
        }
    };

    match code {
        KeyCode::Escape => Some(1),
        KeyCode::Digit1 => Some(2),
        KeyCode::Digit2 => Some(3),
        KeyCode::Digit3 => Some(4),
        KeyCode::Digit4 => Some(5),
        KeyCode::Digit5 => Some(6),
        KeyCode::Digit6 => Some(7),
        KeyCode::Digit7 => Some(8),
        KeyCode::Digit8 => Some(9),
        KeyCode::Digit9 => Some(10),
        KeyCode::Digit0 => Some(11),
        KeyCode::Minus => Some(12),
        KeyCode::Equal => Some(13),
        KeyCode::Backspace => Some(14),
        KeyCode::Tab => Some(15),
        KeyCode::KeyQ => Some(16),
        KeyCode::KeyW => Some(17),
        KeyCode::KeyE => Some(18),
        KeyCode::KeyR => Some(19),
        KeyCode::KeyT => Some(20),
        KeyCode::KeyY => Some(21),
        KeyCode::KeyU => Some(22),
        KeyCode::KeyI => Some(23),
        KeyCode::KeyO => Some(24),
        KeyCode::KeyP => Some(25),
        KeyCode::BracketLeft => Some(26),
        KeyCode::BracketRight => Some(27),
        KeyCode::Enter => Some(28),
        KeyCode::ControlLeft => Some(29),
        KeyCode::KeyA => Some(30),
        KeyCode::KeyS => Some(31),
        KeyCode::KeyD => Some(32),
        KeyCode::KeyF => Some(33),
        KeyCode::KeyG => Some(34),
        KeyCode::KeyH => Some(35),
        KeyCode::KeyJ => Some(36),
        KeyCode::KeyK => Some(37),
        KeyCode::KeyL => Some(38),
        KeyCode::Semicolon => Some(39),
        KeyCode::Quote => Some(40),
        KeyCode::Backquote => Some(41),
        KeyCode::ShiftLeft => Some(42),
        KeyCode::Backslash => Some(43),
        KeyCode::KeyZ => Some(44),
        KeyCode::KeyX => Some(45),
        KeyCode::KeyC => Some(46),
        KeyCode::KeyV => Some(47),
        KeyCode::KeyB => Some(48),
        KeyCode::KeyN => Some(49),
        KeyCode::KeyM => Some(50),
        KeyCode::Comma => Some(51),
        KeyCode::Period => Some(52),
        KeyCode::Slash => Some(53),
        KeyCode::ShiftRight => Some(54),
        KeyCode::NumpadMultiply => Some(55),
        KeyCode::AltLeft => Some(56),
        KeyCode::Space => Some(57),
        KeyCode::CapsLock => Some(58),
        KeyCode::F1 => Some(59),
        KeyCode::F2 => Some(60),
        KeyCode::F3 => Some(61),
        KeyCode::F4 => Some(62),
        KeyCode::F5 => Some(63),
        KeyCode::F6 => Some(64),
        KeyCode::F7 => Some(65),
        KeyCode::F8 => Some(66),
        KeyCode::F9 => Some(67),
        KeyCode::F10 => Some(68),
        KeyCode::NumLock => Some(69),
        KeyCode::ScrollLock => Some(70),
        KeyCode::Numpad7 => Some(71),
        KeyCode::Numpad8 => Some(72),
        KeyCode::Numpad9 => Some(73),
        KeyCode::NumpadSubtract => Some(74),
        KeyCode::Numpad4 => Some(75),
        KeyCode::Numpad5 => Some(76),
        KeyCode::Numpad6 => Some(77),
        KeyCode::NumpadAdd => Some(78),
        KeyCode::Numpad1 => Some(79),
        KeyCode::Numpad2 => Some(80),
        KeyCode::Numpad3 => Some(81),
        KeyCode::Numpad0 => Some(82),
        KeyCode::NumpadDecimal => Some(83),
        KeyCode::Lang5 => Some(85),
        KeyCode::IntlBackslash => Some(86),
        KeyCode::F11 => Some(87),
        KeyCode::F12 => Some(88),
        KeyCode::IntlRo => Some(89),
        KeyCode::Lang3 => Some(90),
        KeyCode::Lang4 => Some(91),
        KeyCode::Convert => Some(92),
        KeyCode::KanaMode => Some(93),
        KeyCode::NonConvert => Some(94),
        KeyCode::NumpadEnter => Some(96),
        KeyCode::ControlRight => Some(97),
        KeyCode::NumpadDivide => Some(98),
        KeyCode::PrintScreen => Some(99),
        KeyCode::AltRight => Some(100),
        KeyCode::Home => Some(102),
        KeyCode::ArrowUp => Some(103),
        KeyCode::PageUp => Some(104),
        KeyCode::ArrowLeft => Some(105),
        KeyCode::ArrowRight => Some(106),
        KeyCode::End => Some(107),
        KeyCode::ArrowDown => Some(108),
        KeyCode::PageDown => Some(109),
        KeyCode::Insert => Some(110),
        KeyCode::Delete => Some(111),
        KeyCode::AudioVolumeMute => Some(113),
        KeyCode::AudioVolumeDown => Some(114),
        KeyCode::AudioVolumeUp => Some(115),
        KeyCode::NumpadEqual => Some(117),
        KeyCode::Pause => Some(119),
        KeyCode::NumpadComma => Some(121),
        KeyCode::Lang1 => Some(122),
        KeyCode::Lang2 => Some(123),
        KeyCode::IntlYen => Some(124),
        KeyCode::SuperLeft => Some(125),
        KeyCode::SuperRight => Some(126),
        KeyCode::ContextMenu => Some(127),
        KeyCode::MediaTrackNext => Some(163),
        KeyCode::MediaPlayPause => Some(164),
        KeyCode::MediaTrackPrevious => Some(165),
        KeyCode::MediaStop => Some(166),
        KeyCode::F13 => Some(183),
        KeyCode::F14 => Some(184),
        KeyCode::F15 => Some(185),
        KeyCode::F16 => Some(186),
        KeyCode::F17 => Some(187),
        KeyCode::F18 => Some(188),
        KeyCode::F19 => Some(189),
        KeyCode::F20 => Some(190),
        KeyCode::F21 => Some(191),
        KeyCode::F22 => Some(192),
        KeyCode::F23 => Some(193),
        KeyCode::F24 => Some(194),
        _ => None,
    }
}

pub fn keysym_to_key(keysym: u32) -> Key {
    use xkbcommon_dl::keysyms;
    Key::Action(match keysym {
        // TTY function keys
        keysyms::BackSpace => Action::Backspace,
        keysyms::Tab => Action::Tab,
        // keysyms::Linefeed => Action::Linefeed,
        keysyms::Clear => Action::Clear,
        keysyms::Return => Action::Enter,
        keysyms::Pause => Action::Pause,
        keysyms::Scroll_Lock => Action::ScrollLock,
        keysyms::Sys_Req => Action::PrintScreen,
        keysyms::Escape => Action::Escape,
        keysyms::Delete => Action::Delete,

        // IME keys
        keysyms::Multi_key => Action::Compose,
        keysyms::Codeinput => Action::CodeInput,
        keysyms::SingleCandidate => Action::SingleCandidate,
        keysyms::MultipleCandidate => Action::AllCandidates,
        keysyms::PreviousCandidate => Action::PreviousCandidate,

        // Japanese keys
        keysyms::Kanji => Action::KanjiMode,
        keysyms::Muhenkan => Action::NonConvert,
        keysyms::Henkan_Mode => Action::Convert,
        keysyms::Romaji => Action::Romaji,
        keysyms::Hiragana => Action::Hiragana,
        keysyms::Hiragana_Katakana => Action::HiraganaKatakana,
        keysyms::Zenkaku => Action::Zenkaku,
        keysyms::Hankaku => Action::Hankaku,
        keysyms::Zenkaku_Hankaku => Action::ZenkakuHankaku,
        // keysyms::Touroku => Action::Touroku,
        // keysyms::Massyo => Action::Massyo,
        keysyms::Kana_Lock => Action::KanaMode,
        keysyms::Kana_Shift => Action::KanaMode,
        keysyms::Eisu_Shift => Action::Alphanumeric,
        keysyms::Eisu_toggle => Action::Alphanumeric,
        // NOTE: The next three items are aliases for values we've already mapped.
        // keysyms::Kanji_Bangou => Action::CodeInput,
        // keysyms::Zen_Koho => Action::AllCandidates,
        // keysyms::Mae_Koho => Action::PreviousCandidate,

        // Cursor control & motion
        keysyms::Home => Action::Home,
        keysyms::Left => Action::ArrowLeft,
        keysyms::Up => Action::ArrowUp,
        keysyms::Right => Action::ArrowRight,
        keysyms::Down => Action::ArrowDown,
        // keysyms::Prior => Action::PageUp,
        keysyms::Page_Up => Action::PageUp,
        // keysyms::Next => Action::PageDown,
        keysyms::Page_Down => Action::PageDown,
        keysyms::End => Action::End,
        // keysyms::Begin => Action::Begin,

        // Misc. functions
        keysyms::Select => Action::Select,
        keysyms::Print => Action::PrintScreen,
        keysyms::Execute => Action::Execute,
        keysyms::Insert => Action::Insert,
        keysyms::Undo => Action::Undo,
        keysyms::Redo => Action::Redo,
        keysyms::Menu => Action::ContextMenu,
        keysyms::Find => Action::Find,
        keysyms::Cancel => Action::Cancel,
        keysyms::Help => Action::Help,
        keysyms::Break => Action::Pause,
        keysyms::Mode_switch => Action::ModeChange,
        // keysyms::script_switch => Action::ModeChange,
        keysyms::Num_Lock => Action::NumLock,

        // Keypad keys
        // keysyms::KP_Space => return Key::Character(" "),
        keysyms::KP_Tab => Action::Tab,
        keysyms::KP_Enter => Action::Enter,
        keysyms::KP_F1 => Action::F1,
        keysyms::KP_F2 => Action::F2,
        keysyms::KP_F3 => Action::F3,
        keysyms::KP_F4 => Action::F4,
        keysyms::KP_Home => Action::Home,
        keysyms::KP_Left => Action::ArrowLeft,
        keysyms::KP_Up => Action::ArrowLeft,
        keysyms::KP_Right => Action::ArrowRight,
        keysyms::KP_Down => Action::ArrowDown,
        // keysyms::KP_Prior => Action::PageUp,
        keysyms::KP_Page_Up => Action::PageUp,
        // keysyms::KP_Next => Action::PageDown,
        keysyms::KP_Page_Down => Action::PageDown,
        keysyms::KP_End => Action::End,
        // This is the key labeled "5" on the numpad when NumLock is off.
        // keysyms::KP_Begin => Action::Begin,
        keysyms::KP_Insert => Action::Insert,
        keysyms::KP_Delete => Action::Delete,
        // keysyms::KP_Equal => Action::Equal,
        // keysyms::KP_Multiply => Action::Multiply,
        // keysyms::KP_Add => Action::Add,
        // keysyms::KP_Separator => Action::Separator,
        // keysyms::KP_Subtract => Action::Subtract,
        // keysyms::KP_Decimal => Action::Decimal,
        // keysyms::KP_Divide => Action::Divide,

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
        keysyms::F1 => Action::F1,
        keysyms::F2 => Action::F2,
        keysyms::F3 => Action::F3,
        keysyms::F4 => Action::F4,
        keysyms::F5 => Action::F5,
        keysyms::F6 => Action::F6,
        keysyms::F7 => Action::F7,
        keysyms::F8 => Action::F8,
        keysyms::F9 => Action::F9,
        keysyms::F10 => Action::F10,
        keysyms::F11 => Action::F11,
        keysyms::F12 => Action::F12,
        keysyms::F13 => Action::F13,
        keysyms::F14 => Action::F14,
        keysyms::F15 => Action::F15,
        keysyms::F16 => Action::F16,
        keysyms::F17 => Action::F17,
        keysyms::F18 => Action::F18,
        keysyms::F19 => Action::F19,
        keysyms::F20 => Action::F20,
        keysyms::F21 => Action::F21,
        keysyms::F22 => Action::F22,
        keysyms::F23 => Action::F23,
        keysyms::F24 => Action::F24,
        keysyms::F25 => Action::F25,
        keysyms::F26 => Action::F26,
        keysyms::F27 => Action::F27,
        keysyms::F28 => Action::F28,
        keysyms::F29 => Action::F29,
        keysyms::F30 => Action::F30,
        keysyms::F31 => Action::F31,
        keysyms::F32 => Action::F32,
        keysyms::F33 => Action::F33,
        keysyms::F34 => Action::F34,
        keysyms::F35 => Action::F35,

        // Modifiers
        keysyms::Shift_L => Action::Shift,
        keysyms::Shift_R => Action::Shift,
        keysyms::Control_L => Action::Control,
        keysyms::Control_R => Action::Control,
        keysyms::Caps_Lock => Action::CapsLock,
        // keysyms::Shift_Lock => Action::ShiftLock,

        // keysyms::Meta_L => Action::Meta,
        // keysyms::Meta_R => Action::Meta,
        keysyms::Alt_L => Action::Alt,
        keysyms::Alt_R => Action::Alt,
        keysyms::Super_L => Action::Super,
        keysyms::Super_R => Action::Super,
        keysyms::Hyper_L => Action::Hyper,
        keysyms::Hyper_R => Action::Hyper,

        // XKB function and modifier keys
        // keysyms::ISO_Lock => Action::IsoLock,
        // keysyms::ISO_Level2_Latch => Action::IsoLevel2Latch,
        keysyms::ISO_Level3_Shift => Action::AltGraph,
        keysyms::ISO_Level3_Latch => Action::AltGraph,
        keysyms::ISO_Level3_Lock => Action::AltGraph,
        // keysyms::ISO_Level5_Shift => Action::IsoLevel5Shift,
        // keysyms::ISO_Level5_Latch => Action::IsoLevel5Latch,
        // keysyms::ISO_Level5_Lock => Action::IsoLevel5Lock,
        // keysyms::ISO_Group_Shift => Action::IsoGroupShift,
        // keysyms::ISO_Group_Latch => Action::IsoGroupLatch,
        // keysyms::ISO_Group_Lock => Action::IsoGroupLock,
        keysyms::ISO_Next_Group => Action::GroupNext,
        // keysyms::ISO_Next_Group_Lock => Action::GroupNextLock,
        keysyms::ISO_Prev_Group => Action::GroupPrevious,
        // keysyms::ISO_Prev_Group_Lock => Action::GroupPreviousLock,
        keysyms::ISO_First_Group => Action::GroupFirst,
        // keysyms::ISO_First_Group_Lock => Action::GroupFirstLock,
        keysyms::ISO_Last_Group => Action::GroupLast,
        // keysyms::ISO_Last_Group_Lock => Action::GroupLastLock,
        //
        keysyms::ISO_Left_Tab => Action::Tab,
        // keysyms::ISO_Move_Line_Up => Action::IsoMoveLineUp,
        // keysyms::ISO_Move_Line_Down => Action::IsoMoveLineDown,
        // keysyms::ISO_Partial_Line_Up => Action::IsoPartialLineUp,
        // keysyms::ISO_Partial_Line_Down => Action::IsoPartialLineDown,
        // keysyms::ISO_Partial_Space_Left => Action::IsoPartialSpaceLeft,
        // keysyms::ISO_Partial_Space_Right => Action::IsoPartialSpaceRight,
        // keysyms::ISO_Set_Margin_Left => Action::IsoSetMarginLeft,
        // keysyms::ISO_Set_Margin_Right => Action::IsoSetMarginRight,
        // keysyms::ISO_Release_Margin_Left => Action::IsoReleaseMarginLeft,
        // keysyms::ISO_Release_Margin_Right => Action::IsoReleaseMarginRight,
        // keysyms::ISO_Release_Both_Margins => Action::IsoReleaseBothMargins,
        // keysyms::ISO_Fast_Cursor_Left => Action::IsoFastCursorLeft,
        // keysyms::ISO_Fast_Cursor_Right => Action::IsoFastCursorRight,
        // keysyms::ISO_Fast_Cursor_Up => Action::IsoFastCursorUp,
        // keysyms::ISO_Fast_Cursor_Down => Action::IsoFastCursorDown,
        // keysyms::ISO_Continuous_Underline => Action::IsoContinuousUnderline,
        // keysyms::ISO_Discontinuous_Underline => Action::IsoDiscontinuousUnderline,
        // keysyms::ISO_Emphasize => Action::IsoEmphasize,
        // keysyms::ISO_Center_Object => Action::IsoCenterObject,
        keysyms::ISO_Enter => Action::Enter,

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
        // keysyms::3270_Duplicate => Action::Duplicate,
        // keysyms::3270_FieldMark => Action::FieldMark,
        // keysyms::3270_Right2 => Action::Right2,
        // keysyms::3270_Left2 => Action::Left2,
        // keysyms::3270_BackTab => Action::BackTab,
        keysyms::_3270_EraseEOF => Action::EraseEof,
        // keysyms::3270_EraseInput => Action::EraseInput,
        // keysyms::3270_Reset => Action::Reset,
        // keysyms::3270_Quit => Action::Quit,
        // keysyms::3270_PA1 => Action::Pa1,
        // keysyms::3270_PA2 => Action::Pa2,
        // keysyms::3270_PA3 => Action::Pa3,
        // keysyms::3270_Test => Action::Test,
        keysyms::_3270_Attn => Action::Attn,
        // keysyms::3270_CursorBlink => Action::CursorBlink,
        // keysyms::3270_AltCursor => Action::AltCursor,
        // keysyms::3270_KeyClick => Action::KeyClick,
        // keysyms::3270_Jump => Action::Jump,
        // keysyms::3270_Ident => Action::Ident,
        // keysyms::3270_Rule => Action::Rule,
        // keysyms::3270_Copy => Action::Copy,
        keysyms::_3270_Play => Action::Play,
        // keysyms::3270_Setup => Action::Setup,
        // keysyms::3270_Record => Action::Record,
        // keysyms::3270_ChangeScreen => Action::ChangeScreen,
        // keysyms::3270_DeleteWord => Action::DeleteWord,
        keysyms::_3270_ExSelect => Action::ExSel,
        keysyms::_3270_CursorSelect => Action::CrSel,
        keysyms::_3270_PrintScreen => Action::PrintScreen,
        keysyms::_3270_Enter => Action::Enter,

        keysyms::space => Action::Space,
        // exclam..Sinh_kunddaliya

        // XFree86
        // keysyms::XF86_ModeLock => Action::ModeLock,

        // XFree86 - Backlight controls
        keysyms::XF86_MonBrightnessUp => Action::BrightnessUp,
        keysyms::XF86_MonBrightnessDown => Action::BrightnessDown,
        // keysyms::XF86_KbdLightOnOff => Action::LightOnOff,
        // keysyms::XF86_KbdBrightnessUp => Action::KeyboardBrightnessUp,
        // keysyms::XF86_KbdBrightnessDown => Action::KeyboardBrightnessDown,

        // XFree86 - "Internet"
        keysyms::XF86_Standby => Action::Standby,
        keysyms::XF86_AudioLowerVolume => Action::AudioVolumeDown,
        keysyms::XF86_AudioRaiseVolume => Action::AudioVolumeUp,
        keysyms::XF86_AudioPlay => Action::MediaPlay,
        keysyms::XF86_AudioStop => Action::MediaStop,
        keysyms::XF86_AudioPrev => Action::MediaTrackPrevious,
        keysyms::XF86_AudioNext => Action::MediaTrackNext,
        keysyms::XF86_HomePage => Action::BrowserHome,
        keysyms::XF86_Mail => Action::LaunchMail,
        // keysyms::XF86_Start => Action::Start,
        keysyms::XF86_Search => Action::BrowserSearch,
        keysyms::XF86_AudioRecord => Action::MediaRecord,

        // XFree86 - PDA
        keysyms::XF86_Calculator => Action::LaunchApplication2,
        // keysyms::XF86_Memo => Action::Memo,
        // keysyms::XF86_ToDoList => Action::ToDoList,
        keysyms::XF86_Calendar => Action::LaunchCalendar,
        keysyms::XF86_PowerDown => Action::Power,
        // keysyms::XF86_ContrastAdjust => Action::AdjustContrast,
        // keysyms::XF86_RockerUp => Action::RockerUp,
        // keysyms::XF86_RockerDown => Action::RockerDown,
        // keysyms::XF86_RockerEnter => Action::RockerEnter,

        // XFree86 - More "Internet"
        keysyms::XF86_Back => Action::BrowserBack,
        keysyms::XF86_Forward => Action::BrowserForward,
        // keysyms::XF86_Stop => Action::Stop,
        keysyms::XF86_Refresh => Action::BrowserRefresh,
        keysyms::XF86_PowerOff => Action::Power,
        keysyms::XF86_WakeUp => Action::WakeUp,
        keysyms::XF86_Eject => Action::Eject,
        keysyms::XF86_ScreenSaver => Action::LaunchScreenSaver,
        keysyms::XF86_WWW => Action::LaunchWebBrowser,
        keysyms::XF86_Sleep => Action::Standby,
        keysyms::XF86_Favorites => Action::BrowserFavorites,
        keysyms::XF86_AudioPause => Action::MediaPause,
        // keysyms::XF86_AudioMedia => Action::AudioMedia,
        keysyms::XF86_MyComputer => Action::LaunchApplication1,
        // keysyms::XF86_VendorHome => Action::VendorHome,
        // keysyms::XF86_LightBulb => Action::LightBulb,
        // keysyms::XF86_Shop => Action::BrowserShop,
        // keysyms::XF86_History => Action::BrowserHistory,
        // keysyms::XF86_OpenURL => Action::OpenUrl,
        // keysyms::XF86_AddFavorite => Action::AddFavorite,
        // keysyms::XF86_HotLinks => Action::HotLinks,
        // keysyms::XF86_BrightnessAdjust => Action::BrightnessAdjust,
        // keysyms::XF86_Finance => Action::BrowserFinance,
        // keysyms::XF86_Community => Action::BrowserCommunity,
        keysyms::XF86_AudioRewind => Action::MediaRewind,
        // keysyms::XF86_BackForward => Key::???,
        // XF86_Launch0..XF86_LaunchF

        // XF86_ApplicationLeft..XF86_CD
        keysyms::XF86_Calculater => Action::LaunchApplication2, // Nice typo, libxkbcommon :)
        // XF86_Clear
        keysyms::XF86_Close => Action::Close,
        keysyms::XF86_Copy => Action::Copy,
        keysyms::XF86_Cut => Action::Cut,
        // XF86_Display..XF86_Documents
        keysyms::XF86_Excel => Action::LaunchSpreadsheet,
        // XF86_Explorer..XF86iTouch
        keysyms::XF86_LogOff => Action::LogOff,
        // XF86_Market..XF86_MenuPB
        keysyms::XF86_MySites => Action::BrowserFavorites,
        keysyms::XF86_New => Action::New,
        // XF86_News..XF86_OfficeHome
        keysyms::XF86_Open => Action::Open,
        // XF86_Option
        keysyms::XF86_Paste => Action::Paste,
        keysyms::XF86_Phone => Action::LaunchPhone,
        // XF86_Q
        keysyms::XF86_Reply => Action::MailReply,
        keysyms::XF86_Reload => Action::BrowserRefresh,
        // XF86_RotateWindows..XF86_RotationKB
        keysyms::XF86_Save => Action::Save,
        // XF86_ScrollUp..XF86_ScrollClick
        keysyms::XF86_Send => Action::MailSend,
        keysyms::XF86_Spell => Action::SpellCheck,
        keysyms::XF86_SplitScreen => Action::SplitScreenToggle,
        // XF86_Support..XF86_User2KB
        keysyms::XF86_Video => Action::LaunchMediaPlayer,
        // XF86_WheelButton
        keysyms::XF86_Word => Action::LaunchWordProcessor,
        // XF86_Xfer
        keysyms::XF86_ZoomIn => Action::ZoomIn,
        keysyms::XF86_ZoomOut => Action::ZoomOut,

        // XF86_Away..XF86_Messenger
        keysyms::XF86_WebCam => Action::LaunchWebCam,
        keysyms::XF86_MailForward => Action::MailForward,
        // XF86_Pictures
        keysyms::XF86_Music => Action::LaunchMusicPlayer,

        // XF86_Battery..XF86_UWB
        //
        keysyms::XF86_AudioForward => Action::MediaFastForward,
        // XF86_AudioRepeat
        keysyms::XF86_AudioRandomPlay => Action::RandomToggle,
        keysyms::XF86_Subtitle => Action::Subtitle,
        keysyms::XF86_AudioCycleTrack => Action::MediaAudioTrack,
        // XF86_CycleAngle..XF86_Blue
        //
        keysyms::XF86_Suspend => Action::Standby,
        keysyms::XF86_Hibernate => Action::Hibernate,
        // XF86_TouchpadToggle..XF86_TouchpadOff
        //
        keysyms::XF86_AudioMute => Action::AudioVolumeMute,

        // XF86_Switch_VT_1..XF86_Switch_VT_12

        // XF86_Ungrab..XF86_ClearGrab
        keysyms::XF86_Next_VMode => Action::VideoModeNext,
        // keysyms::XF86_Prev_VMode => Action::VideoModePrevious,
        // XF86_LogWindowTree..XF86_LogGrabInfo

        // SunFA_Grave..SunFA_Cedilla

        // keysyms::SunF36 => Action::F36 | Action::F11,
        // keysyms::SunF37 => Action::F37 | Action::F12,

        // keysyms::SunSys_Req => Action::PrintScreen,
        // The next couple of xkb (until SunStop) are already handled.
        // SunPrint_Screen..SunPageDown

        // SunUndo..SunFront
        keysyms::SUN_Copy => Action::Copy,
        keysyms::SUN_Open => Action::Open,
        keysyms::SUN_Paste => Action::Paste,
        keysyms::SUN_Cut => Action::Cut,

        // SunPowerSwitch
        keysyms::SUN_AudioLowerVolume => Action::AudioVolumeDown,
        keysyms::SUN_AudioMute => Action::AudioVolumeMute,
        keysyms::SUN_AudioRaiseVolume => Action::AudioVolumeUp,
        // SUN_VideoDegauss
        keysyms::SUN_VideoLowerBrightness => Action::BrightnessDown,
        keysyms::SUN_VideoRaiseBrightness => Action::BrightnessUp,
        // SunPowerSwitchShift
        //
        0 => return Key::Unidentified(NativeKey::Unidentified),
        _ => return Key::Unidentified(NativeKey::Xkb(keysym)),
    })
}

pub fn keysym_location(keysym: u32) -> KeyLocation {
    use xkbcommon_dl::keysyms;
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
