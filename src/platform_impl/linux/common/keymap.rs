//! Convert Wayland keys to winit keys.

use crate::keyboard::{Key, KeyCode, KeyLocation, NativeKeyCode};

// TODO: Do another pass on all of this

pub fn rawkey_to_keycode(rawkey: u32) -> KeyCode {
    // The keycode values are taken from linux/include/uapi/linux/input-event-codes.h, as
    // libxkbcommon's documentation indicates that the keycode values we're getting from it are
    // defined by the Linux kernel. If Winit programs end up being run on other Unix-likes which
    // also use libxkbcommon, then I dearly hope the keycode values mean the same thing.
    //
    // Some of the keycodes are likely superfluous for our purposes, and some are ones which are
    // difficult to test the correctness of, or discover the purpose of. Because of this, they've
    // either been commented out here, or not included at all.
    //
    // TODO: There are probably a couple more unproblematic keycodes to map here.
    match rawkey {
        0 => KeyCode::Unidentified(NativeKeyCode::XKB(0)), // TODO: Is `NativeKeyCode::Unidentified` better?
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
        76 => KeyCode::Numpad4,
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
        240 => KeyCode::Unidentified(NativeKeyCode::Unidentified),
        // 241 => KeyCode::VIDEO_NEXT,
        // 242 => KeyCode::VIDEO_PREV,
        // 243 => KeyCode::BRIGHTNESS_CYCLE,
        // 244 => KeyCode::BRIGHTNESS_AUTO,
        // 245 => KeyCode::DISPLAY_OFF,
        // 246 => KeyCode::WWAN,
        // 247 => KeyCode::RFKILL,
        // 248 => KeyCode::KEY_MICMUTE,
        _ => KeyCode::Unidentified(NativeKeyCode::XKB(rawkey)),
    }
}

pub fn keysym_to_key(keysym: u32) -> Key<'static> {
    use xkbcommon_dl::keysyms;
    match keysym {
        // TTY function keys
        keysyms::XKB_KEY_BackSpace => Key::Backspace,
        keysyms::XKB_KEY_Tab => Key::Tab,
        // keysyms::XKB_KEY_Linefeed => Key::Linefeed,
        keysyms::XKB_KEY_Clear => Key::Clear,
        keysyms::XKB_KEY_Return => Key::Enter,
        // keysyms::XKB_KEY_Pause => Key::Pause,
        keysyms::XKB_KEY_Scroll_Lock => Key::ScrollLock,
        keysyms::XKB_KEY_Sys_Req => Key::PrintScreen,
        keysyms::XKB_KEY_Escape => Key::Escape,
        keysyms::XKB_KEY_Delete => Key::Delete,

        // IME keys
        keysyms::XKB_KEY_Multi_key => Key::Compose,
        keysyms::XKB_KEY_Codeinput => Key::CodeInput,
        keysyms::XKB_KEY_SingleCandidate => Key::SingleCandidate,
        keysyms::XKB_KEY_MultipleCandidate => Key::AllCandidates,
        keysyms::XKB_KEY_PreviousCandidate => Key::PreviousCandidate,

        // Japanese keys
        keysyms::XKB_KEY_Kanji => Key::KanjiMode,
        keysyms::XKB_KEY_Muhenkan => Key::NonConvert,
        keysyms::XKB_KEY_Henkan_Mode => Key::Convert,
        keysyms::XKB_KEY_Romaji => Key::Romaji,
        keysyms::XKB_KEY_Hiragana => Key::Hiragana,
        keysyms::XKB_KEY_Hiragana_Katakana => Key::HiraganaKatakana,
        keysyms::XKB_KEY_Zenkaku => Key::Zenkaku,
        keysyms::XKB_KEY_Hankaku => Key::Hankaku,
        keysyms::XKB_KEY_Zenkaku_Hankaku => Key::ZenkakuHankaku,
        // keysyms::XKB_KEY_Touroku => Key::Touroku,
        // keysyms::XKB_KEY_Massyo => Key::Massyo,
        keysyms::XKB_KEY_Kana_Lock => Key::KanaMode,
        // TODO: This seems a tad perverse, but I'm not really familiar with japanese keyboards.
        //       MDN documents this as a valid mapping, however.
        // keysyms::XKB_KEY_Kana_Shift => Key::KanaMode,
        // TODO: Is this the correct mapping?
        // keysyms::XKB_KEY_Eisu_Shift => Key::Alphanumeric,
        // keysyms::XKB_KEY_Eisu_toggle => Key::Alphanumeric,
        // NOTE: The next three items are aliases for values we've already mapped.
        // keysyms::XKB_KEY_Kanji_Bangou => Key::CodeInput,
        // keysyms::XKB_KEY_Zen_Koho => Key::AllCandidates,
        // keysyms::XKB_KEY_Mae_Koho => Key::PreviousCandidate,

        // Cursor control & motion
        keysyms::XKB_KEY_Home => Key::Home,
        keysyms::XKB_KEY_Left => Key::ArrowLeft,
        keysyms::XKB_KEY_Up => Key::ArrowUp,
        keysyms::XKB_KEY_Right => Key::ArrowRight,
        keysyms::XKB_KEY_Down => Key::ArrowDown,
        // keysyms::XKB_KEY_Prior => Key::PageUp,
        keysyms::XKB_KEY_Page_Up => Key::PageUp,
        // keysyms::XKB_KEY_Next => Key::PageDown,
        keysyms::XKB_KEY_Page_Down => Key::PageDown,
        keysyms::XKB_KEY_End => Key::End,
        // keysyms::XKB_KEY_Begin => Key::Begin,

        // Misc. functions
        keysyms::XKB_KEY_Select => Key::Select,
        keysyms::XKB_KEY_Print => Key::PrintScreen,
        keysyms::XKB_KEY_Execute => Key::Execute,
        keysyms::XKB_KEY_Insert => Key::Insert,
        keysyms::XKB_KEY_Undo => Key::Undo,
        keysyms::XKB_KEY_Redo => Key::Redo,
        keysyms::XKB_KEY_Menu => Key::ContextMenu,
        keysyms::XKB_KEY_Find => Key::Find,
        keysyms::XKB_KEY_Cancel => Key::Cancel,
        keysyms::XKB_KEY_Help => Key::Help,
        keysyms::XKB_KEY_Break => Key::Pause,
        keysyms::XKB_KEY_Mode_switch => Key::ModeChange,
        // keysyms::XKB_KEY_script_switch => Key::ModeChange,
        keysyms::XKB_KEY_Num_Lock => Key::NumLock,

        // Keypad keys
        // keysyms::XKB_KEY_KP_Space => Key::Character(" "),
        keysyms::XKB_KEY_KP_Tab => Key::Tab,
        keysyms::XKB_KEY_KP_Enter => Key::Enter,
        keysyms::XKB_KEY_KP_F1 => Key::F1,
        keysyms::XKB_KEY_KP_F2 => Key::F2,
        keysyms::XKB_KEY_KP_F3 => Key::F3,
        keysyms::XKB_KEY_KP_F4 => Key::F4,
        keysyms::XKB_KEY_KP_Home => Key::Home,
        keysyms::XKB_KEY_KP_Left => Key::ArrowLeft,
        keysyms::XKB_KEY_KP_Up => Key::ArrowLeft,
        keysyms::XKB_KEY_KP_Right => Key::ArrowRight,
        keysyms::XKB_KEY_KP_Down => Key::ArrowDown,
        // keysyms::XKB_KEY_KP_Prior => Key::PageUp,
        keysyms::XKB_KEY_KP_Page_Up => Key::PageUp,
        // keysyms::XKB_KEY_KP_Next => Key::PageDown,
        keysyms::XKB_KEY_KP_Page_Down => Key::PageDown,
        keysyms::XKB_KEY_KP_End => Key::End,
        // keysyms::XKB_KEY_KP_Begin => Key::Begin,
        keysyms::XKB_KEY_KP_Insert => Key::Insert,
        keysyms::XKB_KEY_KP_Delete => Key::Delete,
        // keysyms::XKB_KEY_KP_Equal => Key::Equal,
        // keysyms::XKB_KEY_KP_Multiply => Key::Multiply,
        // keysyms::XKB_KEY_KP_Add => Key::Add,
        // keysyms::XKB_KEY_KP_Separator => Key::Separator,
        // keysyms::XKB_KEY_KP_Subtract => Key::Subtract,
        // keysyms::XKB_KEY_KP_Decimal => Key::Decimal,
        // keysyms::XKB_KEY_KP_Divide => Key::Divide,

        // keysyms::XKB_KEY_KP_0 => Key::Character("0"),
        // keysyms::XKB_KEY_KP_1 => Key::Character("1"),
        // keysyms::XKB_KEY_KP_2 => Key::Character("2"),
        // keysyms::XKB_KEY_KP_3 => Key::Character("3"),
        // keysyms::XKB_KEY_KP_4 => Key::Character("4"),
        // keysyms::XKB_KEY_KP_5 => Key::Character("5"),
        // keysyms::XKB_KEY_KP_6 => Key::Character("6"),
        // keysyms::XKB_KEY_KP_7 => Key::Character("7"),
        // keysyms::XKB_KEY_KP_8 => Key::Character("8"),
        // keysyms::XKB_KEY_KP_9 => Key::Character("9"),

        // Function keys
        keysyms::XKB_KEY_F1 => Key::F1,
        keysyms::XKB_KEY_F2 => Key::F2,
        keysyms::XKB_KEY_F3 => Key::F3,
        keysyms::XKB_KEY_F4 => Key::F4,
        keysyms::XKB_KEY_F5 => Key::F5,
        keysyms::XKB_KEY_F6 => Key::F6,
        keysyms::XKB_KEY_F7 => Key::F7,
        keysyms::XKB_KEY_F8 => Key::F8,
        keysyms::XKB_KEY_F9 => Key::F9,
        keysyms::XKB_KEY_F10 => Key::F10,
        keysyms::XKB_KEY_F11 => Key::F11,
        keysyms::XKB_KEY_F12 => Key::F12,
        keysyms::XKB_KEY_F13 => Key::F13,
        keysyms::XKB_KEY_F14 => Key::F14,
        keysyms::XKB_KEY_F15 => Key::F15,
        keysyms::XKB_KEY_F16 => Key::F16,
        keysyms::XKB_KEY_F17 => Key::F17,
        keysyms::XKB_KEY_F18 => Key::F18,
        keysyms::XKB_KEY_F19 => Key::F19,
        keysyms::XKB_KEY_F20 => Key::F20,
        keysyms::XKB_KEY_F21 => Key::F21,
        keysyms::XKB_KEY_F22 => Key::F22,
        keysyms::XKB_KEY_F23 => Key::F23,
        keysyms::XKB_KEY_F24 => Key::F24,
        keysyms::XKB_KEY_F25 => Key::F25,
        keysyms::XKB_KEY_F26 => Key::F26,
        keysyms::XKB_KEY_F27 => Key::F27,
        keysyms::XKB_KEY_F28 => Key::F28,
        keysyms::XKB_KEY_F29 => Key::F29,
        keysyms::XKB_KEY_F30 => Key::F30,
        keysyms::XKB_KEY_F31 => Key::F31,
        keysyms::XKB_KEY_F32 => Key::F32,
        keysyms::XKB_KEY_F33 => Key::F33,
        keysyms::XKB_KEY_F34 => Key::F34,
        keysyms::XKB_KEY_F35 => Key::F35,

        // Modifiers
        keysyms::XKB_KEY_Shift_L => Key::Shift,
        keysyms::XKB_KEY_Shift_R => Key::Shift,
        keysyms::XKB_KEY_Control_L => Key::Control,
        keysyms::XKB_KEY_Control_R => Key::Control,
        keysyms::XKB_KEY_Caps_Lock => Key::CapsLock,
        // keysyms::XKB_KEY_Shift_Lock => Key::ShiftLock,
        //
        // TODO: Address how renaming `Meta` to `Super` affects this set of keys.
        // keysyms::XKB_KEY_Meta_L => Key::Meta,
        // keysyms::XKB_KEY_Meta_R => Key::Meta,
        keysyms::XKB_KEY_Alt_L => Key::Alt,
        keysyms::XKB_KEY_Alt_R => Key::Alt,
        keysyms::XKB_KEY_Super_L => Key::Super,
        keysyms::XKB_KEY_Super_R => Key::Super,
        keysyms::XKB_KEY_Hyper_L => Key::Hyper,
        keysyms::XKB_KEY_Hyper_R => Key::Hyper,

        // XKB function and modifier keys
        // keysyms::XKB_KEY_ISO_Lock => Key::IsoLock,
        // keysyms::XKB_KEY_ISO_Level2_Latch => Key::IsoLevel2Latch,
        // NOTE: I'm not quite certain if mapping the next 3 values to AltGraph is correct.
        // keysyms::XKB_KEY_ISO_Level3_Shift => Key::AltGraph,
        // keysyms::XKB_KEY_ISO_Level3_Latch => Key::AltGraph,
        // keysyms::XKB_KEY_ISO_Level3_Lock => Key::AltGraph,
        // keysyms::XKB_KEY_ISO_Level5_Shift => Key::IsoLevel5Shift,
        // keysyms::XKB_KEY_ISO_Level5_Latch => Key::IsoLevel5Latch,
        // keysyms::XKB_KEY_ISO_Level5_Lock => Key::IsoLevel5Lock,
        // keysyms::XKB_KEY_ISO_Group_Shift => Key::IsoGroupShift,
        // keysyms::XKB_KEY_ISO_Group_Latch => Key::IsoGroupLatch,
        // keysyms::XKB_KEY_ISO_Group_Lock => Key::IsoGroupLock,
        keysyms::XKB_KEY_ISO_Next_Group => Key::GroupNext,
        // keysyms::XKB_KEY_ISO_Next_Group_Lock => Key::GroupNextLock,
        keysyms::XKB_KEY_ISO_Prev_Group => Key::GroupPrevious,
        // keysyms::XKB_KEY_ISO_Prev_Group_Lock => Key::GroupPreviousLock,
        keysyms::XKB_KEY_ISO_First_Group => Key::GroupFirst,
        // keysyms::XKB_KEY_ISO_First_Group_Lock => Key::GroupFirstLock,
        keysyms::XKB_KEY_ISO_Last_Group => Key::GroupLast,
        // keysyms::XKB_KEY_ISO_Last_Group_Lock => Key::GroupLastLock,
        //
        keysyms::XKB_KEY_ISO_Left_Tab => Key::Tab,
        // keysyms::XKB_KEY_ISO_Move_Line_Up => Key::IsoMoveLineUp,
        // keysyms::XKB_KEY_ISO_Move_Line_Down => Key::IsoMoveLineDown,
        // keysyms::XKB_KEY_ISO_Partial_Line_Up => Key::IsoPartialLineUp,
        // keysyms::XKB_KEY_ISO_Partial_Line_Down => Key::IsoPartialLineDown,
        // keysyms::XKB_KEY_ISO_Partial_Space_Left => Key::IsoPartialSpaceLeft,
        // keysyms::XKB_KEY_ISO_Partial_Space_Right => Key::IsoPartialSpaceRight,
        // keysyms::XKB_KEY_ISO_Set_Margin_Left => Key::IsoSetMarginLeft,
        // keysyms::XKB_KEY_ISO_Set_Margin_Right => Key::IsoSetMarginRight,
        // keysyms::XKB_KEY_ISO_Release_Margin_Left => Key::IsoReleaseMarginLeft,
        // keysyms::XKB_KEY_ISO_Release_Margin_Right => Key::IsoReleaseMarginRight,
        // keysyms::XKB_KEY_ISO_Release_Both_Margins => Key::IsoReleaseBothMargins,
        // keysyms::XKB_KEY_ISO_Fast_Cursor_Left => Key::IsoFastCursorLeft,
        // keysyms::XKB_KEY_ISO_Fast_Cursor_Right => Key::IsoFastCursorRight,
        // keysyms::XKB_KEY_ISO_Fast_Cursor_Up => Key::IsoFastCursorUp,
        // keysyms::XKB_KEY_ISO_Fast_Cursor_Down => Key::IsoFastCursorDown,
        // keysyms::XKB_KEY_ISO_Continuous_Underline => Key::IsoContinuousUnderline,
        // keysyms::XKB_KEY_ISO_Discontinuous_Underline => Key::IsoDiscontinuousUnderline,
        // keysyms::XKB_KEY_ISO_Emphasize => Key::IsoEmphasize,
        // keysyms::XKB_KEY_ISO_Center_Object => Key::IsoCenterObject,
        keysyms::XKB_KEY_ISO_Enter => Key::Enter,

        // XKB_KEY_dead_grave..XKB_KEY_dead_currency

        // XKB_KEY_dead_lowline..XKB_KEY_dead_longsolidusoverlay

        // XKB_KEY_dead_a..XKB_KEY_dead_capital_schwa

        // XKB_KEY_dead_greek

        // XKB_KEY_First_Virtual_Screen..XKB_KEY_Terminate_Server

        // XKB_KEY_AccessX_Enable..XKB_KEY_AudibleBell_Enable

        // XKB_KEY_Pointer_Left..XKB_KEY_Pointer_Drag5

        // XKB_KEY_Pointer_EnableKeys..XKB_KEY_Pointer_DfltBtnPrev

        // XKB_KEY_ch..XKB_KEY_C_H

        // 3270 terminal keys
        // keysyms::XKB_KEY_3270_Duplicate => Key::Duplicate,
        // keysyms::XKB_KEY_3270_FieldMark => Key::FieldMark,
        // keysyms::XKB_KEY_3270_Right2 => Key::Right2,
        // keysyms::XKB_KEY_3270_Left2 => Key::Left2,
        // keysyms::XKB_KEY_3270_BackTab => Key::BackTab,
        keysyms::XKB_KEY_3270_EraseEOF => Key::EraseEof,
        // keysyms::XKB_KEY_3270_EraseInput => Key::EraseInput,
        // keysyms::XKB_KEY_3270_Reset => Key::Reset,
        // keysyms::XKB_KEY_3270_Quit => Key::Quit,
        // keysyms::XKB_KEY_3270_PA1 => Key::Pa1,
        // keysyms::XKB_KEY_3270_PA2 => Key::Pa2,
        // keysyms::XKB_KEY_3270_PA3 => Key::Pa3,
        // keysyms::XKB_KEY_3270_Test => Key::Test,
        keysyms::XKB_KEY_3270_Attn => Key::Attn,
        // keysyms::XKB_KEY_3270_CursorBlink => Key::CursorBlink,
        // keysyms::XKB_KEY_3270_AltCursor => Key::AltCursor,
        // keysyms::XKB_KEY_3270_KeyClick => Key::KeyClick,
        // keysyms::XKB_KEY_3270_Jump => Key::Jump,
        // keysyms::XKB_KEY_3270_Ident => Key::Ident,
        // keysyms::XKB_KEY_3270_Rule => Key::Rule,
        // keysyms::XKB_KEY_3270_Copy => Key::Copy,
        keysyms::XKB_KEY_3270_Play => Key::Play,
        // keysyms::XKB_KEY_3270_Setup => Key::Setup,
        // keysyms::XKB_KEY_3270_Record => Key::Record,
        // keysyms::XKB_KEY_3270_ChangeScreen => Key::ChangeScreen,
        // keysyms::XKB_KEY_3270_DeleteWord => Key::DeleteWord,
        keysyms::XKB_KEY_3270_ExSelect => Key::ExSel,
        keysyms::XKB_KEY_3270_CursorSelect => Key::CrSel,
        keysyms::XKB_KEY_3270_PrintScreen => Key::PrintScreen,
        keysyms::XKB_KEY_3270_Enter => Key::Enter,

        keysyms::XKB_KEY_space => Key::Space,
        // XKB_KEY_exclam..XKB_KEY_Sinh_kunddaliya

        // XFree86
        // keysyms::XKB_KEY_XF86ModeLock => Key::ModeLock,

        // XFree86 - Backlight controls
        keysyms::XKB_KEY_XF86MonBrightnessUp => Key::BrightnessUp,
        keysyms::XKB_KEY_XF86MonBrightnessDown => Key::BrightnessDown,
        // keysyms::XKB_KEY_XF86KbdLightOnOff => Key::LightOnOff,
        // keysyms::XKB_KEY_XF86KbdBrightnessUp => Key::KeyboardBrightnessUp,
        // keysyms::XKB_KEY_XF86KbdBrightnessDown => Key::KeyboardBrightnessDown,

        // XFree86 - "Internet"
        keysyms::XKB_KEY_XF86Standby => Key::Standby,
        keysyms::XKB_KEY_XF86AudioLowerVolume => Key::AudioVolumeDown,
        keysyms::XKB_KEY_XF86AudioRaiseVolume => Key::AudioVolumeUp,
        keysyms::XKB_KEY_XF86AudioPlay => Key::MediaPlay,
        keysyms::XKB_KEY_XF86AudioStop => Key::MediaStop,
        keysyms::XKB_KEY_XF86AudioPrev => Key::MediaTrackPrevious,
        keysyms::XKB_KEY_XF86AudioNext => Key::MediaTrackNext,
        keysyms::XKB_KEY_XF86HomePage => Key::BrowserHome,
        keysyms::XKB_KEY_XF86Mail => Key::LaunchMail,
        // keysyms::XKB_KEY_XF86Start => Key::Start,
        keysyms::XKB_KEY_XF86Search => Key::BrowserSearch,
        keysyms::XKB_KEY_XF86AudioRecord => Key::MediaRecord,

        // XFree86 - PDA
        keysyms::XKB_KEY_XF86Calculator => Key::LaunchApplication2,
        // keysyms::XKB_KEY_XF86Memo => Key::Memo,
        // keysyms::XKB_KEY_XF86ToDoList => Key::ToDoList,
        keysyms::XKB_KEY_XF86Calendar => Key::LaunchCalendar,
        keysyms::XKB_KEY_XF86PowerDown => Key::Power,
        // keysyms::XKB_KEY_XF86ContrastAdjust => Key::AdjustContrast,
        // keysyms::XKB_KEY_XF86RockerUp => Key::RockerUp, // TODO: Use Key::ArrowUp?
        // keysyms::XKB_KEY_XF86RockerDown => Key::RockerDown, // TODO: Use Key::ArrowDown?
        // keysyms::XKB_KEY_XF86RockerEnter => Key::RockerEnter, // TODO: Use Key::Enter?

        // XFree86 - More "Internet"
        keysyms::XKB_KEY_XF86Back => Key::BrowserBack,
        keysyms::XKB_KEY_XF86Forward => Key::BrowserForward,
        // keysyms::XKB_KEY_XF86Stop => Key::Stop,
        keysyms::XKB_KEY_XF86Refresh => Key::BrowserRefresh,
        keysyms::XKB_KEY_XF86PowerOff => Key::Power,
        keysyms::XKB_KEY_XF86WakeUp => Key::WakeUp,
        keysyms::XKB_KEY_XF86Eject => Key::Eject,
        keysyms::XKB_KEY_XF86ScreenSaver => Key::LaunchScreenSaver,
        keysyms::XKB_KEY_XF86WWW => Key::LaunchWebBrowser,
        keysyms::XKB_KEY_XF86Sleep => Key::Standby,
        keysyms::XKB_KEY_XF86Favorites => Key::BrowserFavorites,
        keysyms::XKB_KEY_XF86AudioPause => Key::MediaPause,
        // keysyms::XKB_KEY_XF86AudioMedia => Key::AudioMedia,
        keysyms::XKB_KEY_XF86MyComputer => Key::LaunchApplication1,
        // keysyms::XKB_KEY_XF86VendorHome => Key::VendorHome,
        // keysyms::XKB_KEY_XF86LightBulb => Key::LightBulb,
        // keysyms::XKB_KEY_XF86Shop => Key::BrowserShop,
        // keysyms::XKB_KEY_XF86History => Key::BrowserHistory,
        // keysyms::XKB_KEY_XF86OpenURL => Key::OpenUrl,
        // keysyms::XKB_KEY_XF86AddFavorite => Key::AddFavorite,
        // keysyms::XKB_KEY_XF86HotLinks => Key::HotLinks,
        // keysyms::XKB_KEY_XF86BrightnessAdjust => Key::BrightnessAdjust,
        // keysyms::XKB_KEY_XF86Finance => Key::BrowserFinance,
        // keysyms::XKB_KEY_XF86Community => Key::BrowserCommunity,
        keysyms::XKB_KEY_XF86AudioRewind => Key::MediaRewind,
        // keysyms::XKB_KEY_XF86BackForward => Key::???,
        // XKB_KEY_XF86Launch0..XKB_KEY_XF86LaunchF

        // XKB_KEY_XF86ApplicationLeft..XKB_KEY_XF86CD
        keysyms::XKB_KEY_XF86Calculater => Key::LaunchApplication2, // This must be a typo, right?
        // XKB_KEY_XF86Clear
        keysyms::XKB_KEY_XF86Close => Key::Close,
        keysyms::XKB_KEY_XF86Copy => Key::Copy,
        keysyms::XKB_KEY_XF86Cut => Key::Cut,
        // XKB_KEY_XF86Display..XKB_KEY_XF86Documents
        keysyms::XKB_KEY_XF86Excel => Key::LaunchSpreadsheet,
        // XKB_KEY_XF86Explorer..XKB_KEY_XF86iTouch
        keysyms::XKB_KEY_XF86LogOff => Key::LogOff,
        // XKB_KEY_XF86Market..XKB_KEY_XF86MenuPB
        keysyms::XKB_KEY_XF86MySites => Key::BrowserFavorites,
        keysyms::XKB_KEY_XF86New => Key::New,
        // XKB_KEY_XF86News..XKB_KEY_XF86OfficeHome
        keysyms::XKB_KEY_XF86Open => Key::Open,
        // XKB_KEY_XF86Option
        keysyms::XKB_KEY_XF86Paste => Key::Paste,
        keysyms::XKB_KEY_XF86Phone => Key::LaunchPhone,
        // XKB_KEY_XF86Q
        keysyms::XKB_KEY_XF86Reply => Key::MailReply,
        keysyms::XKB_KEY_XF86Reload => Key::BrowserRefresh,
        // XKB_KEY_XF86RotateWindows..XKB_KEY_XF86RotationKB
        keysyms::XKB_KEY_XF86Save => Key::Save,
        // XKB_KEY_XF86ScrollUp..XKB_KEY_XF86ScrollClick
        keysyms::XKB_KEY_XF86Send => Key::MailSend,
        keysyms::XKB_KEY_XF86Spell => Key::SpellCheck,
        keysyms::XKB_KEY_XF86SplitScreen => Key::SplitScreenToggle,
        // XKB_KEY_XF86Support..XKB_KEY_XF86User2KB
        keysyms::XKB_KEY_XF86Video => Key::LaunchMediaPlayer,
        // XKB_KEY_XF86WheelButton
        keysyms::XKB_KEY_XF86Word => Key::LaunchWordProcessor,
        // XKB_KEY_XF86Xfer
        keysyms::XKB_KEY_XF86ZoomIn => Key::ZoomIn,
        keysyms::XKB_KEY_XF86ZoomOut => Key::ZoomOut,

        // XKB_KEY_XF86Away..XKB_KEY_XF86Messenger
        keysyms::XKB_KEY_XF86WebCam => Key::LaunchWebCam,
        keysyms::XKB_KEY_XF86MailForward => Key::MailForward,
        // XKB_KEY_XF86Pictures
        keysyms::XKB_KEY_XF86Music => Key::LaunchMusicPlayer,

        // XKB_KEY_XF86Battery..XKB_KEY_XF86UWB
        //
        keysyms::XKB_KEY_XF86AudioForward => Key::MediaFastForward,
        // XKB_KEY_XF86AudioRepeat
        keysyms::XKB_KEY_XF86AudioRandomPlay => Key::RandomToggle,
        keysyms::XKB_KEY_XF86Subtitle => Key::Subtitle,
        keysyms::XKB_KEY_XF86AudioCycleTrack => Key::MediaAudioTrack,
        // XKB_KEY_XF86CycleAngle..XKB_KEY_XF86Blue
        //
        keysyms::XKB_KEY_XF86Suspend => Key::Standby,
        keysyms::XKB_KEY_XF86Hibernate => Key::Hibernate,
        // XKB_KEY_XF86TouchpadToggle..XKB_KEY_XF86TouchpadOff
        //
        keysyms::XKB_KEY_XF86AudioMute => Key::AudioVolumeMute,

        // XKB_KEY_XF86Switch_VT_1..XKB_KEY_XF86Switch_VT_12

        // XKB_KEY_XF86Ungrab..XKB_KEY_XF86ClearGrab
        keysyms::XKB_KEY_XF86Next_VMode => Key::VideoModeNext,
        // keysyms::XKB_KEY_XF86Prev_VMode => Key::VideoModePrevious,
        // XKB_KEY_XF86LogWindowTree..XKB_KEY_XF86LogGrabInfo

        // XKB_KEY_SunFA_Grave..XKB_KEY_SunFA_Cedilla

        // keysyms::XKB_KEY_SunF36 => Key::F36 | Key::F11,
        // keysyms::XKB_KEY_SunF37 => Key::F37 | Key::F12,

        // keysyms::XKB_KEY_SunSys_Req => Key::PrintScreen,
        // The next couple of xkb (until XKB_KEY_SunStop) are already handled.
        // XKB_KEY_SunPrint_Screen..XKB_KEY_SunPageDown

        // XKB_KEY_SunUndo..XKB_KEY_SunFront
        keysyms::XKB_KEY_SunCopy => Key::Copy,
        keysyms::XKB_KEY_SunOpen => Key::Open,
        keysyms::XKB_KEY_SunPaste => Key::Paste,
        keysyms::XKB_KEY_SunCut => Key::Cut,

        // XKB_KEY_SunPowerSwitch
        keysyms::XKB_KEY_SunAudioLowerVolume => Key::AudioVolumeDown,
        keysyms::XKB_KEY_SunAudioMute => Key::AudioVolumeMute,
        keysyms::XKB_KEY_SunAudioRaiseVolume => Key::AudioVolumeUp,
        // XKB_KEY_SunVideoDegauss
        keysyms::XKB_KEY_SunVideoLowerBrightness => Key::BrightnessDown,
        keysyms::XKB_KEY_SunVideoRaiseBrightness => Key::BrightnessUp,
        // XKB_KEY_SunPowerSwitchShift
        //
        0 => Key::Unidentified(NativeKeyCode::Unidentified),
        _ => Key::Unidentified(NativeKeyCode::XKB(keysym)),
    }
}

pub fn keysym_location(keysym: u32) -> KeyLocation {
    use xkbcommon_dl::keysyms;
    match keysym {
        keysyms::XKB_KEY_Shift_L
        | keysyms::XKB_KEY_Control_L
        | keysyms::XKB_KEY_Meta_L
        | keysyms::XKB_KEY_Alt_L
        | keysyms::XKB_KEY_Super_L
        | keysyms::XKB_KEY_Hyper_L => KeyLocation::Left,
        keysyms::XKB_KEY_Shift_R
        | keysyms::XKB_KEY_Control_R
        | keysyms::XKB_KEY_Meta_R
        | keysyms::XKB_KEY_Alt_R
        | keysyms::XKB_KEY_Super_R
        | keysyms::XKB_KEY_Hyper_R => KeyLocation::Right,
        keysyms::XKB_KEY_KP_0
        | keysyms::XKB_KEY_KP_1
        | keysyms::XKB_KEY_KP_2
        | keysyms::XKB_KEY_KP_3
        | keysyms::XKB_KEY_KP_4
        | keysyms::XKB_KEY_KP_5
        | keysyms::XKB_KEY_KP_6
        | keysyms::XKB_KEY_KP_7
        | keysyms::XKB_KEY_KP_8
        | keysyms::XKB_KEY_KP_9
        | keysyms::XKB_KEY_KP_Space
        | keysyms::XKB_KEY_KP_Tab
        | keysyms::XKB_KEY_KP_Enter
        | keysyms::XKB_KEY_KP_F1
        | keysyms::XKB_KEY_KP_F2
        | keysyms::XKB_KEY_KP_F3
        | keysyms::XKB_KEY_KP_F4
        | keysyms::XKB_KEY_KP_Home
        | keysyms::XKB_KEY_KP_Left
        | keysyms::XKB_KEY_KP_Up
        | keysyms::XKB_KEY_KP_Right
        | keysyms::XKB_KEY_KP_Down
        | keysyms::XKB_KEY_KP_Page_Up
        | keysyms::XKB_KEY_KP_Page_Down
        | keysyms::XKB_KEY_KP_End
        | keysyms::XKB_KEY_KP_Begin
        | keysyms::XKB_KEY_KP_Insert
        | keysyms::XKB_KEY_KP_Delete
        | keysyms::XKB_KEY_KP_Equal
        | keysyms::XKB_KEY_KP_Multiply
        | keysyms::XKB_KEY_KP_Add
        | keysyms::XKB_KEY_KP_Separator
        | keysyms::XKB_KEY_KP_Subtract
        | keysyms::XKB_KEY_KP_Decimal
        | keysyms::XKB_KEY_KP_Divide => KeyLocation::Numpad,
        _ => KeyLocation::Standard,
    }
}
