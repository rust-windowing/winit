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
    use xkbcommon::xkb;
    match keysym {
        // TTY function keys
        xkb::KEY_BackSpace => Key::Backspace,
        xkb::KEY_Tab => Key::Tab,
        // xkb::KEY_Linefeed => Key::Linefeed,
        xkb::KEY_Clear => Key::Clear,
        xkb::KEY_Return => Key::Enter,
        // xkb::KEY_Pause => Key::Pause,
        xkb::KEY_Scroll_Lock => Key::ScrollLock,
        xkb::KEY_Sys_Req => Key::PrintScreen,
        xkb::KEY_Escape => Key::Escape,
        xkb::KEY_Delete => Key::Delete,

        // IME keys
        xkb::KEY_Multi_key => Key::Compose,
        xkb::KEY_Codeinput => Key::CodeInput,
        xkb::KEY_SingleCandidate => Key::SingleCandidate,
        xkb::KEY_MultipleCandidate => Key::AllCandidates,
        xkb::KEY_PreviousCandidate => Key::PreviousCandidate,

        // Japanese keys
        xkb::KEY_Kanji => Key::KanjiMode,
        xkb::KEY_Muhenkan => Key::NonConvert,
        xkb::KEY_Henkan_Mode => Key::Convert,
        xkb::KEY_Romaji => Key::Romaji,
        xkb::KEY_Hiragana => Key::Hiragana,
        xkb::KEY_Hiragana_Katakana => Key::HiraganaKatakana,
        xkb::KEY_Zenkaku => Key::Zenkaku,
        xkb::KEY_Hankaku => Key::Hankaku,
        xkb::KEY_Zenkaku_Hankaku => Key::ZenkakuHankaku,
        // xkb::KEY_Touroku => Key::Touroku,
        // xkb::KEY_Massyo => Key::Massyo,
        xkb::KEY_Kana_Lock => Key::KanaMode,
        // TODO: This seems a tad perverse, but I'm not really familiar with japanese keyboards.
        //       MDN documents this as a valid mapping, however.
        // xkb::KEY_Kana_Shift => Key::KanaMode,
        // TODO: Is this the correct mapping?
        // xkb::KEY_Eisu_Shift => Key::Alphanumeric,
        // xkb::KEY_Eisu_toggle => Key::Alphanumeric,
        // NOTE: The next three items are aliases for values we've already mapped.
        // xkb::KEY_Kanji_Bangou => Key::CodeInput,
        // xkb::KEY_Zen_Koho => Key::AllCandidates,
        // xkb::KEY_Mae_Koho => Key::PreviousCandidate,

        // Cursor control & motion
        xkb::KEY_Home => Key::Home,
        xkb::KEY_Left => Key::ArrowLeft,
        xkb::KEY_Up => Key::ArrowUp,
        xkb::KEY_Right => Key::ArrowRight,
        xkb::KEY_Down => Key::ArrowDown,
        // xkb::KEY_Prior => Key::PageUp,
        xkb::KEY_Page_Up => Key::PageUp,
        // xkb::KEY_Next => Key::PageDown,
        xkb::KEY_End => Key::End,
        // xkb::KEY_Begin => Key::Begin,

        // Misc. functions
        xkb::KEY_Select => Key::Select,
        xkb::KEY_Print => Key::PrintScreen,
        xkb::KEY_Execute => Key::Execute,
        xkb::KEY_Insert => Key::Insert,
        xkb::KEY_Undo => Key::Undo,
        xkb::KEY_Redo => Key::Redo,
        xkb::KEY_Menu => Key::ContextMenu,
        xkb::KEY_Find => Key::Find,
        xkb::KEY_Cancel => Key::Cancel,
        xkb::KEY_Help => Key::Help,
        xkb::KEY_Break => Key::Pause,
        xkb::KEY_Mode_switch => Key::ModeChange,
        // xkb::KEY_script_switch => Key::ModeChange,
        xkb::KEY_Num_Lock => Key::NumLock,

        // Keypad keys
        // xkb::KEY_KP_Space => Key::Character(" "),
        xkb::KEY_KP_Tab => Key::Tab,
        xkb::KEY_KP_Enter => Key::Enter,
        xkb::KEY_KP_F1 => Key::F1,
        xkb::KEY_KP_F2 => Key::F2,
        xkb::KEY_KP_F3 => Key::F3,
        xkb::KEY_KP_F4 => Key::F4,
        xkb::KEY_KP_Home => Key::Home,
        xkb::KEY_KP_Left => Key::ArrowLeft,
        xkb::KEY_KP_Up => Key::ArrowLeft,
        xkb::KEY_KP_Right => Key::ArrowRight,
        xkb::KEY_KP_Down => Key::ArrowDown,
        // xkb::KEY_KP_Prior => Key::PageUp,
        xkb::KEY_KP_Page_Up => Key::PageUp,
        // xkb::KEY_KP_Next => Key::PageDown,
        xkb::KEY_KP_Page_Down => Key::PageDown,
        xkb::KEY_KP_End => Key::End,
        // xkb::KEY_KP_Begin => Key::Begin,
        xkb::KEY_KP_Insert => Key::Insert,
        xkb::KEY_KP_Delete => Key::Delete,
        // xkb::KEY_KP_Equal => Key::Equal,
        // xkb::KEY_KP_Multiply => Key::Multiply,
        // xkb::KEY_KP_Add => Key::Add,
        // xkb::KEY_KP_Separator => Key::Separator,
        // xkb::KEY_KP_Subtract => Key::Subtract,
        // xkb::KEY_KP_Decimal => Key::Decimal,
        // xkb::KEY_KP_Divide => Key::Divide,

        // xkb::KEY_KP_0 => Key::Character("0"),
        // xkb::KEY_KP_1 => Key::Character("1"),
        // xkb::KEY_KP_2 => Key::Character("2"),
        // xkb::KEY_KP_3 => Key::Character("3"),
        // xkb::KEY_KP_4 => Key::Character("4"),
        // xkb::KEY_KP_5 => Key::Character("5"),
        // xkb::KEY_KP_6 => Key::Character("6"),
        // xkb::KEY_KP_7 => Key::Character("7"),
        // xkb::KEY_KP_8 => Key::Character("8"),
        // xkb::KEY_KP_9 => Key::Character("9"),

        // Function keys
        xkb::KEY_F1 => Key::F1,
        xkb::KEY_F2 => Key::F2,
        xkb::KEY_F3 => Key::F3,
        xkb::KEY_F4 => Key::F4,
        xkb::KEY_F5 => Key::F5,
        xkb::KEY_F6 => Key::F6,
        xkb::KEY_F7 => Key::F7,
        xkb::KEY_F8 => Key::F8,
        xkb::KEY_F9 => Key::F9,
        xkb::KEY_F10 => Key::F10,
        xkb::KEY_F11 => Key::F11,
        xkb::KEY_F12 => Key::F12,
        xkb::KEY_F13 => Key::F13,
        xkb::KEY_F14 => Key::F14,
        xkb::KEY_F15 => Key::F15,
        xkb::KEY_F16 => Key::F16,
        xkb::KEY_F17 => Key::F17,
        xkb::KEY_F18 => Key::F18,
        xkb::KEY_F19 => Key::F19,
        xkb::KEY_F20 => Key::F20,
        xkb::KEY_F21 => Key::F21,
        xkb::KEY_F22 => Key::F22,
        xkb::KEY_F23 => Key::F23,
        xkb::KEY_F24 => Key::F24,
        xkb::KEY_F25 => Key::F25,
        xkb::KEY_F26 => Key::F26,
        xkb::KEY_F27 => Key::F27,
        xkb::KEY_F28 => Key::F28,
        xkb::KEY_F29 => Key::F29,
        xkb::KEY_F30 => Key::F30,
        xkb::KEY_F31 => Key::F31,
        xkb::KEY_F32 => Key::F32,
        xkb::KEY_F33 => Key::F33,
        xkb::KEY_F34 => Key::F34,
        xkb::KEY_F35 => Key::F35,

        // Modifiers
        xkb::KEY_Shift_L => Key::Shift,
        xkb::KEY_Shift_R => Key::Shift,
        xkb::KEY_Control_L => Key::Control,
        xkb::KEY_Control_R => Key::Control,
        xkb::KEY_Caps_Lock => Key::CapsLock,
        // xkb::KEY_Shift_Lock => Key::ShiftLock,

        // NOTE: The key xkb calls "Meta" is called "Super" by Winit, and vice versa.
        //       This is a tad confusing, but these keys have different names depending on who you ask.
        xkb::KEY_Meta_L => Key::Super,
        xkb::KEY_Meta_R => Key::Super,
        xkb::KEY_Alt_L => Key::Alt,
        xkb::KEY_Alt_R => Key::Alt,
        xkb::KEY_Super_L => Key::Meta,
        xkb::KEY_Super_R => Key::Meta,
        xkb::KEY_Hyper_L => Key::Hyper,
        xkb::KEY_Hyper_R => Key::Hyper,

        // XKB function and modifier keys
        // xkb::KEY_ISO_Lock => Key::IsoLock,
        // xkb::KEY_ISO_Level2_Latch => Key::IsoLevel2Latch,
        // NOTE: I'm not quite certain if mapping the next 3 values to AltGraph is correct.
        // xkb::KEY_ISO_Level3_Shift => Key::AltGraph,
        // xkb::KEY_ISO_Level3_Latch => Key::AltGraph,
        // xkb::KEY_ISO_Level3_Lock => Key::AltGraph,
        // xkb::KEY_ISO_Level5_Shift => Key::IsoLevel5Shift,
        // xkb::KEY_ISO_Level5_Latch => Key::IsoLevel5Latch,
        // xkb::KEY_ISO_Level5_Lock => Key::IsoLevel5Lock,
        // xkb::KEY_ISO_Group_Shift => Key::IsoGroupShift,
        // xkb::KEY_ISO_Group_Latch => Key::IsoGroupLatch,
        // xkb::KEY_ISO_Group_Lock => Key::IsoGroupLock,
        xkb::KEY_ISO_Next_Group => Key::GroupNext,
        // xkb::KEY_ISO_Next_Group_Lock => Key::GroupNextLock,
        xkb::KEY_ISO_Prev_Group => Key::GroupPrevious,
        // xkb::KEY_ISO_Prev_Group_Lock => Key::GroupPreviousLock,
        xkb::KEY_ISO_First_Group => Key::GroupFirst,
        // xkb::KEY_ISO_First_Group_Lock => Key::GroupFirstLock,
        xkb::KEY_ISO_Last_Group => Key::GroupLast,
        // xkb::KEY_ISO_Last_Group_Lock => Key::GroupLastLock,
        //
        xkb::KEY_ISO_Left_Tab => Key::Tab,
        // xkb::KEY_ISO_Move_Line_Up => Key::IsoMoveLineUp,
        // xkb::KEY_ISO_Move_Line_Down => Key::IsoMoveLineDown,
        // xkb::KEY_ISO_Partial_Line_Up => Key::IsoPartialLineUp,
        // xkb::KEY_ISO_Partial_Line_Down => Key::IsoPartialLineDown,
        // xkb::KEY_ISO_Partial_Space_Left => Key::IsoPartialSpaceLeft,
        // xkb::KEY_ISO_Partial_Space_Right => Key::IsoPartialSpaceRight,
        // xkb::KEY_ISO_Set_Margin_Left => Key::IsoSetMarginLeft,
        // xkb::KEY_ISO_Set_Margin_Right => Key::IsoSetMarginRight,
        // xkb::KEY_ISO_Release_Margin_Left => Key::IsoReleaseMarginLeft,
        // xkb::KEY_ISO_Release_Margin_Right => Key::IsoReleaseMarginRight,
        // xkb::KEY_ISO_Release_Both_Margins => Key::IsoReleaseBothMargins,
        // xkb::KEY_ISO_Fast_Cursor_Left => Key::IsoFastCursorLeft,
        // xkb::KEY_ISO_Fast_Cursor_Right => Key::IsoFastCursorRight,
        // xkb::KEY_ISO_Fast_Cursor_Up => Key::IsoFastCursorUp,
        // xkb::KEY_ISO_Fast_Cursor_Down => Key::IsoFastCursorDown,
        // xkb::KEY_ISO_Continuous_Underline => Key::IsoContinuousUnderline,
        // xkb::KEY_ISO_Discontinuous_Underline => Key::IsoDiscontinuousUnderline,
        // xkb::KEY_ISO_Emphasize => Key::IsoEmphasize,
        // xkb::KEY_ISO_Center_Object => Key::IsoCenterObject,
        xkb::KEY_ISO_Enter => Key::Enter,

        // KEY_dead_grave..KEY_dead_currency

        // KEY_dead_lowline..KEY_dead_longsolidusoverlay

        // KEY_dead_a..KEY_dead_capital_schwa

        // KEY_dead_greek

        // KEY_First_Virtual_Screen..KEY_Terminate_Server

        // KEY_AccessX_Enable..KEY_AudibleBell_Enable

        // KEY_Pointer_Left..KEY_Pointer_Drag5

        // KEY_Pointer_EnableKeys..KEY_Pointer_DfltBtnPrev

        // KEY_ch..KEY_C_H

        // 3270 terminal keys
        // xkb::KEY_3270_Duplicate => Key::Duplicate,
        // xkb::KEY_3270_FieldMark => Key::FieldMark,
        // xkb::KEY_3270_Right2 => Key::Right2,
        // xkb::KEY_3270_Left2 => Key::Left2,
        // xkb::KEY_3270_BackTab => Key::BackTab,
        xkb::KEY_3270_EraseEOF => Key::EraseEof,
        // xkb::KEY_3270_EraseInput => Key::EraseInput,
        // xkb::KEY_3270_Reset => Key::Reset,
        // xkb::KEY_3270_Quit => Key::Quit,
        // xkb::KEY_3270_PA1 => Key::Pa1,
        // xkb::KEY_3270_PA2 => Key::Pa2,
        // xkb::KEY_3270_PA3 => Key::Pa3,
        // xkb::KEY_3270_Test => Key::Test,
        xkb::KEY_3270_Attn => Key::Attn,
        // xkb::KEY_3270_CursorBlink => Key::CursorBlink,
        // xkb::KEY_3270_AltCursor => Key::AltCursor,
        // xkb::KEY_3270_KeyClick => Key::KeyClick,
        // xkb::KEY_3270_Jump => Key::Jump,
        // xkb::KEY_3270_Ident => Key::Ident,
        // xkb::KEY_3270_Rule => Key::Rule,
        // xkb::KEY_3270_Copy => Key::Copy,
        xkb::KEY_3270_Play => Key::Play,
        // xkb::KEY_3270_Setup => Key::Setup,
        // xkb::KEY_3270_Record => Key::Record,
        // xkb::KEY_3270_ChangeScreen => Key::ChangeScreen,
        // xkb::KEY_3270_DeleteWord => Key::DeleteWord,
        xkb::KEY_3270_ExSelect => Key::ExSel,
        xkb::KEY_3270_CursorSelect => Key::CrSel,
        xkb::KEY_3270_PrintScreen => Key::PrintScreen,
        xkb::KEY_3270_Enter => Key::Enter,

        xkb::KEY_space => Key::Space,
        // KEY_exclam..KEY_Sinh_kunddaliya

        // XFree86
        // xkb::KEY_XF86ModeLock => Key::ModeLock,

        // XFree86 - Backlight controls
        xkb::KEY_XF86MonBrightnessUp => Key::BrightnessUp,
        xkb::KEY_XF86MonBrightnessDown => Key::BrightnessDown,
        // xkb::KEY_XF86KbdLightOnOff => Key::LightOnOff,
        // xkb::KEY_XF86KbdBrightnessUp => Key::KeyboardBrightnessUp,
        // xkb::KEY_XF86KbdBrightnessDown => Key::KeyboardBrightnessDown,

        // XFree86 - "Internet"
        xkb::KEY_XF86Standby => Key::Standby,
        xkb::KEY_XF86AudioLowerVolume => Key::AudioVolumeDown,
        xkb::KEY_XF86AudioRaiseVolume => Key::AudioVolumeUp,
        xkb::KEY_XF86AudioPlay => Key::MediaPlay,
        xkb::KEY_XF86AudioStop => Key::MediaStop,
        xkb::KEY_XF86AudioPrev => Key::MediaTrackPrevious,
        xkb::KEY_XF86AudioNext => Key::MediaTrackNext,
        xkb::KEY_XF86HomePage => Key::BrowserHome,
        xkb::KEY_XF86Mail => Key::LaunchMail,
        // xkb::KEY_XF86Start => Key::Start,
        xkb::KEY_XF86Search => Key::BrowserSearch,
        xkb::KEY_XF86AudioRecord => Key::MediaRecord,

        // XFree86 - PDA
        xkb::KEY_XF86Calculator => Key::LaunchApplication2,
        // xkb::KEY_XF86Memo => Key::Memo,
        // xkb::KEY_XF86ToDoList => Key::ToDoList,
        xkb::KEY_XF86Calendar => Key::LaunchCalendar,
        xkb::KEY_XF86PowerDown => Key::Power,
        // xkb::KEY_XF86ContrastAdjust => Key::AdjustContrast,
        // xkb::KEY_XF86RockerUp => Key::RockerUp, // TODO: Use Key::ArrowUp?
        // xkb::KEY_XF86RockerDown => Key::RockerDown, // TODO: Use Key::ArrowDown?
        // xkb::KEY_XF86RockerEnter => Key::RockerEnter, // TODO: Use Key::Enter?

        // XFree86 - More "Internet"
        xkb::KEY_XF86Back => Key::BrowserBack,
        xkb::KEY_XF86Forward => Key::BrowserForward,
        // xkb::KEY_XF86Stop => Key::Stop,
        xkb::KEY_XF86Refresh => Key::BrowserRefresh,
        xkb::KEY_XF86PowerOff => Key::Power,
        xkb::KEY_XF86WakeUp => Key::WakeUp,
        xkb::KEY_XF86Eject => Key::Eject,
        xkb::KEY_XF86ScreenSaver => Key::LaunchScreenSaver,
        xkb::KEY_XF86WWW => Key::LaunchWebBrowser,
        xkb::KEY_XF86Sleep => Key::Standby,
        xkb::KEY_XF86Favorites => Key::BrowserFavorites,
        xkb::KEY_XF86AudioPause => Key::MediaPause,
        // xkb::KEY_XF86AudioMedia => Key::AudioMedia,
        xkb::KEY_XF86MyComputer => Key::LaunchApplication1,
        // xkb::KEY_XF86VendorHome => Key::VendorHome,
        // xkb::KEY_XF86LightBulb => Key::LightBulb,
        // xkb::KEY_XF86Shop => Key::BrowserShop,
        // xkb::KEY_XF86History => Key::BrowserHistory,
        // xkb::KEY_XF86OpenURL => Key::OpenUrl,
        // xkb::KEY_XF86AddFavorite => Key::AddFavorite,
        // xkb::KEY_XF86HotLinks => Key::HotLinks,
        // xkb::KEY_XF86BrightnessAdjust => Key::BrightnessAdjust,
        // xkb::KEY_XF86Finance => Key::BrowserFinance,
        // xkb::KEY_XF86Community => Key::BrowserCommunity,
        xkb::KEY_XF86AudioRewind => Key::MediaRewind,
        // xkb::KEY_XF86BackForward => Key::???,
        // KEY_XF86Launch0..KEY_XF86LaunchF

        // KEY_XF86ApplicationLeft..KEY_XF86CD
        xkb::KEY_XF86Calculater => Key::LaunchApplication2, // This must be a typo, right?
        // KEY_XF86Clear
        xkb::KEY_XF86Close => Key::Close,
        xkb::KEY_XF86Copy => Key::Copy,
        xkb::KEY_XF86Cut => Key::Cut,
        // KEY_XF86Display..KEY_XF86Documents
        xkb::KEY_XF86Excel => Key::LaunchSpreadsheet,
        // KEY_XF86Explorer..KEY_XF86iTouch
        xkb::KEY_XF86LogOff => Key::LogOff,
        // KEY_XF86Market..KEY_XF86MenuPB
        xkb::KEY_XF86MySites => Key::BrowserFavorites,
        xkb::KEY_XF86New => Key::New,
        // KEY_XF86News..KEY_XF86OfficeHome
        xkb::KEY_XF86Open => Key::Open,
        // KEY_XF86Option
        xkb::KEY_XF86Paste => Key::Paste,
        xkb::KEY_XF86Phone => Key::LaunchPhone,
        // KEY_XF86Q
        xkb::KEY_XF86Reply => Key::MailReply,
        xkb::KEY_XF86Reload => Key::BrowserRefresh,
        // KEY_XF86RotateWindows..KEY_XF86RotationKB
        xkb::KEY_XF86Save => Key::Save,
        // KEY_XF86ScrollUp..KEY_XF86ScrollClick
        xkb::KEY_XF86Send => Key::MailSend,
        xkb::KEY_XF86Spell => Key::SpellCheck,
        xkb::KEY_XF86SplitScreen => Key::SplitScreenToggle,
        // KEY_XF86Support..KEY_XF86User2KB
        xkb::KEY_XF86Video => Key::LaunchMediaPlayer,
        // KEY_XF86WheelButton
        xkb::KEY_XF86Word => Key::LaunchWordProcessor,
        // KEY_XF86Xfer
        xkb::KEY_XF86ZoomIn => Key::ZoomIn,
        xkb::KEY_XF86ZoomOut => Key::ZoomOut,

        // KEY_XF86Away..KEY_XF86Messenger
        xkb::KEY_XF86WebCam => Key::LaunchWebCam,
        xkb::KEY_XF86MailForward => Key::MailForward,
        // KEY_XF86Pictures
        xkb::KEY_XF86Music => Key::LaunchMusicPlayer,

        // KEY_XF86Battery..KEY_XF86UWB
        //
        xkb::KEY_XF86AudioForward => Key::MediaFastForward,
        // KEY_XF86AudioRepeat
        xkb::KEY_XF86AudioRandomPlay => Key::RandomToggle,
        xkb::KEY_XF86Subtitle => Key::Subtitle,
        xkb::KEY_XF86AudioCycleTrack => Key::MediaAudioTrack,
        // KEY_XF86CycleAngle..KEY_XF86Blue
        //
        xkb::KEY_XF86Suspend => Key::Standby,
        xkb::KEY_XF86Hibernate => Key::Hibernate,
        // KEY_XF86TouchpadToggle..KEY_XF86TouchpadOff
        //
        xkb::KEY_XF86AudioMute => Key::AudioVolumeMute,

        // KEY_XF86Switch_VT_1..KEY_XF86Switch_VT_12

        // KEY_XF86Ungrab..KEY_XF86ClearGrab
        xkb::KEY_XF86Next_VMode => Key::VideoModeNext,
        // xkb::KEY_XF86Prev_VMode => Key::VideoModePrevious,
        // KEY_XF86LogWindowTree..KEY_XF86LogGrabInfo

        // KEY_SunFA_Grave..KEY_SunFA_Cedilla

        // xkb::KEY_SunF36 => Key::F36 | Key::F11,
        // xkb::KEY_SunF37 => Key::F37 | Key::F12,

        // xkb::KEY_SunSys_Req => Key::PrintScreen,
        // The next couple of xkb (until KEY_SunStop) are already handled.
        // KEY_SunPrint_Screen..KEY_SunPageDown

        // KEY_SunUndo..KEY_SunFront
        xkb::KEY_SunCopy => Key::Copy,
        xkb::KEY_SunOpen => Key::Open,
        xkb::KEY_SunPaste => Key::Paste,
        xkb::KEY_SunCut => Key::Cut,

        // KEY_SunPowerSwitch
        xkb::KEY_SunAudioLowerVolume => Key::AudioVolumeDown,
        xkb::KEY_SunAudioMute => Key::AudioVolumeMute,
        xkb::KEY_SunAudioRaiseVolume => Key::AudioVolumeUp,
        // KEY_SunVideoDegauss
        xkb::KEY_SunVideoLowerBrightness => Key::BrightnessDown,
        xkb::KEY_SunVideoRaiseBrightness => Key::BrightnessUp,
        // KEY_SunPowerSwitchShift
        //
        _ => Key::Unidentified(NativeKeyCode::XKB(keysym)),
    }
}

pub fn keysym_location(keysym: u32) -> KeyLocation {
    use xkbcommon::xkb;
    match keysym {
        xkb::KEY_Shift_L
        | xkb::KEY_Control_L
        | xkb::KEY_Meta_L
        | xkb::KEY_Alt_L
        | xkb::KEY_Super_L
        | xkb::KEY_Hyper_L => KeyLocation::Left,
        xkb::KEY_Shift_R
        | xkb::KEY_Control_R
        | xkb::KEY_Meta_R
        | xkb::KEY_Alt_R
        | xkb::KEY_Super_R
        | xkb::KEY_Hyper_R => KeyLocation::Right,
        xkb::KEY_KP_0
        | xkb::KEY_KP_1
        | xkb::KEY_KP_2
        | xkb::KEY_KP_3
        | xkb::KEY_KP_4
        | xkb::KEY_KP_5
        | xkb::KEY_KP_6
        | xkb::KEY_KP_7
        | xkb::KEY_KP_8
        | xkb::KEY_KP_9
        | xkb::KEY_KP_Space
        | xkb::KEY_KP_Tab
        | xkb::KEY_KP_Enter
        | xkb::KEY_KP_F1
        | xkb::KEY_KP_F2
        | xkb::KEY_KP_F3
        | xkb::KEY_KP_F4
        | xkb::KEY_KP_Home
        | xkb::KEY_KP_Left
        | xkb::KEY_KP_Up
        | xkb::KEY_KP_Right
        | xkb::KEY_KP_Down
        | xkb::KEY_KP_Page_Up
        | xkb::KEY_KP_Page_Down
        | xkb::KEY_KP_End
        | xkb::KEY_KP_Begin
        | xkb::KEY_KP_Insert
        | xkb::KEY_KP_Delete
        | xkb::KEY_KP_Equal
        | xkb::KEY_KP_Multiply
        | xkb::KEY_KP_Add
        | xkb::KEY_KP_Separator
        | xkb::KEY_KP_Subtract
        | xkb::KEY_KP_Decimal
        | xkb::KEY_KP_Divide => KeyLocation::Numpad,
        _ => KeyLocation::Standard,
    }
}
