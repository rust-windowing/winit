//! Convert Wayland keys to winit keys.
#![allow(non_upper_case_globals)]

use crate::event::VirtualKeyCode;

// TTY function keys, cleverly chosen to map to ASCII, for convenience of
// programming, but could have been arbitrary (at the cost of lookup
// tables in client code).

pub const XKB_KEY_BackSpace: u32 = 0xff08; // Back space, back char
pub const XKB_KEY_Tab: u32 = 0xff09;
pub const XKB_KEY_Return: u32 = 0xff0d; // Return, enter
pub const XKB_KEY_Pause: u32 = 0xff13; // Pause, hold
pub const XKB_KEY_Scroll_Lock: u32 = 0xff14;
pub const XKB_KEY_Sys_Req: u32 = 0xff15;
pub const XKB_KEY_Escape: u32 = 0xff1b;
pub const XKB_KEY_Delete: u32 = 0xffff; // Delete, rubout
pub const XKB_KEY_Caps_Lock: u32 = 0xffe5;

// International & multi-key character composition

pub const XKB_KEY_Multi_key: u32 = 0xff20; // Multi-key character compose

// Japanese keyboard support

pub const XKB_KEY_Kanji: u32 = 0xff21; // Kanji, Kanji convert

// Cursor control & motion

pub const XKB_KEY_Home: u32 = 0xff50;
pub const XKB_KEY_Left: u32 = 0xff51; // Move left, left arrow
pub const XKB_KEY_Up: u32 = 0xff52; // Move up, up arrow
pub const XKB_KEY_Right: u32 = 0xff53; // Move right, right arrow
pub const XKB_KEY_Down: u32 = 0xff54; // Move down, down arrow
pub const XKB_KEY_Page_Up: u32 = 0xff55;
pub const XKB_KEY_Page_Down: u32 = 0xff56;
pub const XKB_KEY_End: u32 = 0xff57; // EOL

// Misc functions

pub const XKB_KEY_Print: u32 = 0xff61;
pub const XKB_KEY_Insert: u32 = 0xff63; // Insert, insert here
pub const XKB_KEY_Num_Lock: u32 = 0xff7f;

// Keypad functions, keypad numbers cleverly chosen to map to ASCII

pub const XKB_KEY_KP_Enter: u32 = 0xff8d; // Enter
pub const XKB_KEY_KP_Home: u32 = 0xff95;
pub const XKB_KEY_KP_Left: u32 = 0xff96;
pub const XKB_KEY_KP_Up: u32 = 0xff97;
pub const XKB_KEY_KP_Right: u32 = 0xff98;
pub const XKB_KEY_KP_Down: u32 = 0xff99;
pub const XKB_KEY_KP_Page_Up: u32 = 0xff9a;
pub const XKB_KEY_KP_Page_Down: u32 = 0xff9b;
pub const XKB_KEY_KP_End: u32 = 0xff9c;
pub const XKB_KEY_KP_Equal: u32 = 0xffbd; // Equals
pub const XKB_KEY_KP_Multiply: u32 = 0xffaa;
pub const XKB_KEY_KP_Add: u32 = 0xffab;
pub const XKB_KEY_KP_Separator: u32 = 0xffac; // Separator, often comma
pub const XKB_KEY_KP_Subtract: u32 = 0xffad;
pub const XKB_KEY_KP_Decimal: u32 = 0xffae;
pub const XKB_KEY_KP_Divide: u32 = 0xffaf;

pub const XKB_KEY_KP_0: u32 = 0xffb0;
pub const XKB_KEY_KP_1: u32 = 0xffb1;
pub const XKB_KEY_KP_2: u32 = 0xffb2;
pub const XKB_KEY_KP_3: u32 = 0xffb3;
pub const XKB_KEY_KP_4: u32 = 0xffb4;
pub const XKB_KEY_KP_5: u32 = 0xffb5;
pub const XKB_KEY_KP_6: u32 = 0xffb6;
pub const XKB_KEY_KP_7: u32 = 0xffb7;
pub const XKB_KEY_KP_8: u32 = 0xffb8;
pub const XKB_KEY_KP_9: u32 = 0xffb9;

// Auxiliary functions; note the duplicate definitions for left and right
// function keys;  Sun keyboards and a few other manufacturers have such
// function key groups on the left and/or right sides of the keyboard.
// We've not found a keyboard with more than 35 function keys total.

pub const XKB_KEY_F1: u32 = 0xffbe;
pub const XKB_KEY_F2: u32 = 0xffbf;
pub const XKB_KEY_F3: u32 = 0xffc0;
pub const XKB_KEY_F4: u32 = 0xffc1;
pub const XKB_KEY_F5: u32 = 0xffc2;
pub const XKB_KEY_F6: u32 = 0xffc3;
pub const XKB_KEY_F7: u32 = 0xffc4;
pub const XKB_KEY_F8: u32 = 0xffc5;
pub const XKB_KEY_F9: u32 = 0xffc6;
pub const XKB_KEY_F10: u32 = 0xffc7;
pub const XKB_KEY_F11: u32 = 0xffc8;
pub const XKB_KEY_F12: u32 = 0xffc9;
pub const XKB_KEY_F13: u32 = 0xffca;
pub const XKB_KEY_F14: u32 = 0xffcb;
pub const XKB_KEY_F15: u32 = 0xffcc;
pub const XKB_KEY_F16: u32 = 0xffcd;
pub const XKB_KEY_F17: u32 = 0xffce;
pub const XKB_KEY_F18: u32 = 0xffcf;
pub const XKB_KEY_F19: u32 = 0xffd0;
pub const XKB_KEY_F20: u32 = 0xffd1;
pub const XKB_KEY_F21: u32 = 0xffd2;
pub const XKB_KEY_F22: u32 = 0xffd3;
pub const XKB_KEY_F23: u32 = 0xffd4;
pub const XKB_KEY_F24: u32 = 0xffd5;

// Modifiers

pub const XKB_KEY_Shift_L: u32 = 0xffe1; // Left shift
pub const XKB_KEY_Shift_R: u32 = 0xffe2; // Right shift
pub const XKB_KEY_Control_L: u32 = 0xffe3; // Left control
pub const XKB_KEY_Control_R: u32 = 0xffe4; // Right control

pub const XKB_KEY_Meta_L: u32 = 0xffe7; // Left meta
pub const XKB_KEY_Meta_R: u32 = 0xffe8; // Right meta
pub const XKB_KEY_Alt_L: u32 = 0xffe9; // Left alt
pub const XKB_KEY_Alt_R: u32 = 0xffea; // Right alt
pub const XKB_KEY_Super_L: u32 = 0xffeb; // Left super
pub const XKB_KEY_Super_R: u32 = 0xffec; // Right super

pub const XKB_KEY_ISO_Left_Tab: u32 = 0xfe20;

// Latin 1
// (ISO/IEC 8859-1 = Unicode U+0020..U+00FF)
// Byte 3 = 0

pub const XKB_KEY_space: u32 = 0x0020; // U+0020 SPACE
pub const XKB_KEY_apostrophe: u32 = 0x0027; // U+0027 APOSTROPHE
pub const XKB_KEY_asterisk: u32 = 0x002a; // U+002A ASTERISK
pub const XKB_KEY_plus: u32 = 0x002b; // U+002B PLUS SIGN
pub const XKB_KEY_comma: u32 = 0x002c; // U+002C COMMA
pub const XKB_KEY_minus: u32 = 0x002d; // U+002D HYPHEN-MINUS
pub const XKB_KEY_period: u32 = 0x002e; // U+002E FULL STOP
pub const XKB_KEY_slash: u32 = 0x002f; // U+002F SOLIDUS
pub const XKB_KEY_0: u32 = 0x0030; // U+0030 DIGIT ZERO
pub const XKB_KEY_1: u32 = 0x0031; // U+0031 DIGIT ONE
pub const XKB_KEY_2: u32 = 0x0032; // U+0032 DIGIT TWO
pub const XKB_KEY_3: u32 = 0x0033; // U+0033 DIGIT THREE
pub const XKB_KEY_4: u32 = 0x0034; // U+0034 DIGIT FOUR
pub const XKB_KEY_5: u32 = 0x0035; // U+0035 DIGIT FIVE
pub const XKB_KEY_6: u32 = 0x0036; // U+0036 DIGIT SIX
pub const XKB_KEY_7: u32 = 0x0037; // U+0037 DIGIT SEVEN
pub const XKB_KEY_8: u32 = 0x0038; // U+0038 DIGIT EIGHT
pub const XKB_KEY_9: u32 = 0x0039; // U+0039 DIGIT NINE
pub const XKB_KEY_colon: u32 = 0x003a; // U+003A COLON
pub const XKB_KEY_semicolon: u32 = 0x003b; // U+003B SEMICOLON
pub const XKB_KEY_equal: u32 = 0x003d; // U+003D EQUALS SIGN
pub const XKB_KEY_at: u32 = 0x0040; // U+0040 COMMERCIAL AT
pub const XKB_KEY_A: u32 = 0x0041; // U+0041 LATIN CAPITAL LETTER A
pub const XKB_KEY_B: u32 = 0x0042; // U+0042 LATIN CAPITAL LETTER B
pub const XKB_KEY_C: u32 = 0x0043; // U+0043 LATIN CAPITAL LETTER C
pub const XKB_KEY_D: u32 = 0x0044; // U+0044 LATIN CAPITAL LETTER D
pub const XKB_KEY_E: u32 = 0x0045; // U+0045 LATIN CAPITAL LETTER E
pub const XKB_KEY_F: u32 = 0x0046; // U+0046 LATIN CAPITAL LETTER F
pub const XKB_KEY_G: u32 = 0x0047; // U+0047 LATIN CAPITAL LETTER G
pub const XKB_KEY_H: u32 = 0x0048; // U+0048 LATIN CAPITAL LETTER H
pub const XKB_KEY_I: u32 = 0x0049; // U+0049 LATIN CAPITAL LETTER I
pub const XKB_KEY_J: u32 = 0x004a; // U+004A LATIN CAPITAL LETTER J
pub const XKB_KEY_K: u32 = 0x004b; // U+004B LATIN CAPITAL LETTER K
pub const XKB_KEY_L: u32 = 0x004c; // U+004C LATIN CAPITAL LETTER L
pub const XKB_KEY_M: u32 = 0x004d; // U+004D LATIN CAPITAL LETTER M
pub const XKB_KEY_N: u32 = 0x004e; // U+004E LATIN CAPITAL LETTER N
pub const XKB_KEY_O: u32 = 0x004f; // U+004F LATIN CAPITAL LETTER O
pub const XKB_KEY_P: u32 = 0x0050; // U+0050 LATIN CAPITAL LETTER P
pub const XKB_KEY_Q: u32 = 0x0051; // U+0051 LATIN CAPITAL LETTER Q
pub const XKB_KEY_R: u32 = 0x0052; // U+0052 LATIN CAPITAL LETTER R
pub const XKB_KEY_S: u32 = 0x0053; // U+0053 LATIN CAPITAL LETTER S
pub const XKB_KEY_T: u32 = 0x0054; // U+0054 LATIN CAPITAL LETTER T
pub const XKB_KEY_U: u32 = 0x0055; // U+0055 LATIN CAPITAL LETTER U
pub const XKB_KEY_V: u32 = 0x0056; // U+0056 LATIN CAPITAL LETTER V
pub const XKB_KEY_W: u32 = 0x0057; // U+0057 LATIN CAPITAL LETTER W
pub const XKB_KEY_X: u32 = 0x0058; // U+0058 LATIN CAPITAL LETTER X
pub const XKB_KEY_Y: u32 = 0x0059; // U+0059 LATIN CAPITAL LETTER Y
pub const XKB_KEY_Z: u32 = 0x005a; // U+005A LATIN CAPITAL LETTER Z
pub const XKB_KEY_bracketleft: u32 = 0x005b; // U+005B LEFT SQUARE BRACKET
pub const XKB_KEY_backslash: u32 = 0x005c; // U+005C REVERSE SOLIDUS
pub const XKB_KEY_bracketright: u32 = 0x005d; // U+005D RIGHT SQUARE BRACKET
pub const XKB_KEY_underscore: u32 = 0x005f; // U+005F LOW LINE
pub const XKB_KEY_grave: u32 = 0x0060; // U+0060 GRAVE ACCENT
pub const XKB_KEY_a: u32 = 0x0061; // U+0061 LATIN SMALL LETTER A
pub const XKB_KEY_b: u32 = 0x0062; // U+0062 LATIN SMALL LETTER B
pub const XKB_KEY_c: u32 = 0x0063; // U+0063 LATIN SMALL LETTER C
pub const XKB_KEY_d: u32 = 0x0064; // U+0064 LATIN SMALL LETTER D
pub const XKB_KEY_e: u32 = 0x0065; // U+0065 LATIN SMALL LETTER E
pub const XKB_KEY_f: u32 = 0x0066; // U+0066 LATIN SMALL LETTER F
pub const XKB_KEY_g: u32 = 0x0067; // U+0067 LATIN SMALL LETTER G
pub const XKB_KEY_h: u32 = 0x0068; // U+0068 LATIN SMALL LETTER H
pub const XKB_KEY_i: u32 = 0x0069; // U+0069 LATIN SMALL LETTER I
pub const XKB_KEY_j: u32 = 0x006a; // U+006A LATIN SMALL LETTER J
pub const XKB_KEY_k: u32 = 0x006b; // U+006B LATIN SMALL LETTER K
pub const XKB_KEY_l: u32 = 0x006c; // U+006C LATIN SMALL LETTER L
pub const XKB_KEY_m: u32 = 0x006d; // U+006D LATIN SMALL LETTER M
pub const XKB_KEY_n: u32 = 0x006e; // U+006E LATIN SMALL LETTER N
pub const XKB_KEY_o: u32 = 0x006f; // U+006F LATIN SMALL LETTER O
pub const XKB_KEY_p: u32 = 0x0070; // U+0070 LATIN SMALL LETTER P
pub const XKB_KEY_q: u32 = 0x0071; // U+0071 LATIN SMALL LETTER Q
pub const XKB_KEY_r: u32 = 0x0072; // U+0072 LATIN SMALL LETTER R
pub const XKB_KEY_s: u32 = 0x0073; // U+0073 LATIN SMALL LETTER S
pub const XKB_KEY_t: u32 = 0x0074; // U+0074 LATIN SMALL LETTER T
pub const XKB_KEY_u: u32 = 0x0075; // U+0075 LATIN SMALL LETTER U
pub const XKB_KEY_v: u32 = 0x0076; // U+0076 LATIN SMALL LETTER V
pub const XKB_KEY_w: u32 = 0x0077; // U+0077 LATIN SMALL LETTER W
pub const XKB_KEY_x: u32 = 0x0078; // U+0078 LATIN SMALL LETTER X
pub const XKB_KEY_y: u32 = 0x0079; // U+0079 LATIN SMALL LETTER Y
pub const XKB_KEY_z: u32 = 0x007a; // U+007A LATIN SMALL LETTER Z
pub const XKB_KEY_yen: u32 = 0x00a5; // U+00A5 YEN SIGN
pub const XKB_KEY_caret: u32 = 0x0afc; // U+2038 CARET

// Keys found on some "Internet" keyboards.

pub const XKB_KEY_XF86AudioLowerVolume: u32 = 0x1008FF11; // Volume control down
pub const XKB_KEY_XF86AudioMute: u32 = 0x1008FF12; // Mute sound from the system
pub const XKB_KEY_XF86AudioRaiseVolume: u32 = 0x1008FF13; // Volume control up
pub const XKB_KEY_XF86AudioPrev: u32 = 0x1008FF16; // Previous track
pub const XKB_KEY_XF86AudioNext: u32 = 0x1008FF17; // Next track
pub const XKB_KEY_XF86Mail: u32 = 0x1008FF19; // Invoke user's mail program

// These are sometimes found on PDA's (e.g. Palm, PocketPC or elsewhere)
pub const XKB_KEY_XF86Calculator: u32 = 0x1008FF1D; // Invoke calculator program
pub const XKB_KEY_XF86PowerOff: u32 = 0x1008FF2A; // Power off system entirely
pub const XKB_KEY_XF86Sleep: u32 = 0x1008FF2F; // Put system to sleep
pub const XKB_KEY_XF86Copy: u32 = 0x1008FF57; // Copy selection
pub const XKB_KEY_XF86Cut: u32 = 0x1008FF58; // Cut selection
pub const XKB_KEY_XF86Paste: u32 = 0x1008FF6D; // Paste
pub const XKB_KEY_XF86AudioStop: u32 = 0x1008FF15;
pub const XKB_KEY_XF86MyComputer: u32 = 0x1008FF33;
pub const XKB_KEY_XF86Stop: u32 = 0x1008FF28;
pub const XKB_KEY_XF86WakeUp: u32 = 0x1008FF2B;
pub const XKB_KEY_XF86Favorites: u32 = 0x1008FF30;
pub const XKB_KEY_XF86HomePage: u32 = 0x1008FF18;
pub const XKB_KEY_XF86Refresh: u32 = 0x1008FF29;

pub fn keysym_to_vkey(keysym: u32) -> Option<VirtualKeyCode> {
    match keysym {
        // Numbers.
        XKB_KEY_1 => Some(VirtualKeyCode::Key1),
        XKB_KEY_2 => Some(VirtualKeyCode::Key2),
        XKB_KEY_3 => Some(VirtualKeyCode::Key3),
        XKB_KEY_4 => Some(VirtualKeyCode::Key4),
        XKB_KEY_5 => Some(VirtualKeyCode::Key5),
        XKB_KEY_6 => Some(VirtualKeyCode::Key6),
        XKB_KEY_7 => Some(VirtualKeyCode::Key7),
        XKB_KEY_8 => Some(VirtualKeyCode::Key8),
        XKB_KEY_9 => Some(VirtualKeyCode::Key9),
        XKB_KEY_0 => Some(VirtualKeyCode::Key0),
        // Letters.
        XKB_KEY_A | XKB_KEY_a => Some(VirtualKeyCode::A),
        XKB_KEY_B | XKB_KEY_b => Some(VirtualKeyCode::B),
        XKB_KEY_C | XKB_KEY_c => Some(VirtualKeyCode::C),
        XKB_KEY_D | XKB_KEY_d => Some(VirtualKeyCode::D),
        XKB_KEY_E | XKB_KEY_e => Some(VirtualKeyCode::E),
        XKB_KEY_F | XKB_KEY_f => Some(VirtualKeyCode::F),
        XKB_KEY_G | XKB_KEY_g => Some(VirtualKeyCode::G),
        XKB_KEY_H | XKB_KEY_h => Some(VirtualKeyCode::H),
        XKB_KEY_I | XKB_KEY_i => Some(VirtualKeyCode::I),
        XKB_KEY_J | XKB_KEY_j => Some(VirtualKeyCode::J),
        XKB_KEY_K | XKB_KEY_k => Some(VirtualKeyCode::K),
        XKB_KEY_L | XKB_KEY_l => Some(VirtualKeyCode::L),
        XKB_KEY_M | XKB_KEY_m => Some(VirtualKeyCode::M),
        XKB_KEY_N | XKB_KEY_n => Some(VirtualKeyCode::N),
        XKB_KEY_O | XKB_KEY_o => Some(VirtualKeyCode::O),
        XKB_KEY_P | XKB_KEY_p => Some(VirtualKeyCode::P),
        XKB_KEY_Q | XKB_KEY_q => Some(VirtualKeyCode::Q),
        XKB_KEY_R | XKB_KEY_r => Some(VirtualKeyCode::R),
        XKB_KEY_S | XKB_KEY_s => Some(VirtualKeyCode::S),
        XKB_KEY_T | XKB_KEY_t => Some(VirtualKeyCode::T),
        XKB_KEY_U | XKB_KEY_u => Some(VirtualKeyCode::U),
        XKB_KEY_V | XKB_KEY_v => Some(VirtualKeyCode::V),
        XKB_KEY_W | XKB_KEY_w => Some(VirtualKeyCode::W),
        XKB_KEY_X | XKB_KEY_x => Some(VirtualKeyCode::X),
        XKB_KEY_Y | XKB_KEY_y => Some(VirtualKeyCode::Y),
        XKB_KEY_Z | XKB_KEY_z => Some(VirtualKeyCode::Z),
        // Escape.
        XKB_KEY_Escape => Some(VirtualKeyCode::Escape),
        // Function keys.
        XKB_KEY_F1 => Some(VirtualKeyCode::F1),
        XKB_KEY_F2 => Some(VirtualKeyCode::F2),
        XKB_KEY_F3 => Some(VirtualKeyCode::F3),
        XKB_KEY_F4 => Some(VirtualKeyCode::F4),
        XKB_KEY_F5 => Some(VirtualKeyCode::F5),
        XKB_KEY_F6 => Some(VirtualKeyCode::F6),
        XKB_KEY_F7 => Some(VirtualKeyCode::F7),
        XKB_KEY_F8 => Some(VirtualKeyCode::F8),
        XKB_KEY_F9 => Some(VirtualKeyCode::F9),
        XKB_KEY_F10 => Some(VirtualKeyCode::F10),
        XKB_KEY_F11 => Some(VirtualKeyCode::F11),
        XKB_KEY_F12 => Some(VirtualKeyCode::F12),
        XKB_KEY_F13 => Some(VirtualKeyCode::F13),
        XKB_KEY_F14 => Some(VirtualKeyCode::F14),
        XKB_KEY_F15 => Some(VirtualKeyCode::F15),
        XKB_KEY_F16 => Some(VirtualKeyCode::F16),
        XKB_KEY_F17 => Some(VirtualKeyCode::F17),
        XKB_KEY_F18 => Some(VirtualKeyCode::F18),
        XKB_KEY_F19 => Some(VirtualKeyCode::F19),
        XKB_KEY_F20 => Some(VirtualKeyCode::F20),
        XKB_KEY_F21 => Some(VirtualKeyCode::F21),
        XKB_KEY_F22 => Some(VirtualKeyCode::F22),
        XKB_KEY_F23 => Some(VirtualKeyCode::F23),
        XKB_KEY_F24 => Some(VirtualKeyCode::F24),
        // Flow control.
        XKB_KEY_Print => Some(VirtualKeyCode::Snapshot),
        XKB_KEY_Scroll_Lock => Some(VirtualKeyCode::Scroll),
        XKB_KEY_Sys_Req => Some(VirtualKeyCode::Sysrq),
        XKB_KEY_Pause => Some(VirtualKeyCode::Pause),
        XKB_KEY_Insert => Some(VirtualKeyCode::Insert),
        XKB_KEY_Home => Some(VirtualKeyCode::Home),
        XKB_KEY_Delete => Some(VirtualKeyCode::Delete),
        XKB_KEY_End => Some(VirtualKeyCode::End),
        XKB_KEY_Page_Down => Some(VirtualKeyCode::PageDown),
        XKB_KEY_Page_Up => Some(VirtualKeyCode::PageUp),
        // Arrows.
        XKB_KEY_Left => Some(VirtualKeyCode::Left),
        XKB_KEY_Up => Some(VirtualKeyCode::Up),
        XKB_KEY_Right => Some(VirtualKeyCode::Right),
        XKB_KEY_Down => Some(VirtualKeyCode::Down),

        XKB_KEY_BackSpace => Some(VirtualKeyCode::Back),
        XKB_KEY_Return => Some(VirtualKeyCode::Return),
        XKB_KEY_space => Some(VirtualKeyCode::Space),

        XKB_KEY_Multi_key => Some(VirtualKeyCode::Compose),
        XKB_KEY_caret => Some(VirtualKeyCode::Caret),

        // Keypad.
        XKB_KEY_Num_Lock => Some(VirtualKeyCode::Numlock),
        XKB_KEY_KP_0 => Some(VirtualKeyCode::Numpad0),
        XKB_KEY_KP_1 => Some(VirtualKeyCode::Numpad1),
        XKB_KEY_KP_2 => Some(VirtualKeyCode::Numpad2),
        XKB_KEY_KP_3 => Some(VirtualKeyCode::Numpad3),
        XKB_KEY_KP_4 => Some(VirtualKeyCode::Numpad4),
        XKB_KEY_KP_5 => Some(VirtualKeyCode::Numpad5),
        XKB_KEY_KP_6 => Some(VirtualKeyCode::Numpad6),
        XKB_KEY_KP_7 => Some(VirtualKeyCode::Numpad7),
        XKB_KEY_KP_8 => Some(VirtualKeyCode::Numpad8),
        XKB_KEY_KP_9 => Some(VirtualKeyCode::Numpad9),
        // Misc.
        // => Some(VirtualKeyCode::AbntC1),
        // => Some(VirtualKeyCode::AbntC2),
        XKB_KEY_plus => Some(VirtualKeyCode::Plus),
        XKB_KEY_apostrophe => Some(VirtualKeyCode::Apostrophe),
        // => Some(VirtualKeyCode::Apps),
        XKB_KEY_at => Some(VirtualKeyCode::At),
        // => Some(VirtualKeyCode::Ax),
        XKB_KEY_backslash => Some(VirtualKeyCode::Backslash),
        XKB_KEY_XF86Calculator => Some(VirtualKeyCode::Calculator),
        XKB_KEY_Caps_Lock => Some(VirtualKeyCode::Capital),
        XKB_KEY_colon => Some(VirtualKeyCode::Colon),
        XKB_KEY_comma => Some(VirtualKeyCode::Comma),
        // => Some(VirtualKeyCode::Convert),
        XKB_KEY_equal => Some(VirtualKeyCode::Equals),
        XKB_KEY_grave => Some(VirtualKeyCode::Grave),
        // => Some(VirtualKeyCode::Kana),
        XKB_KEY_Kanji => Some(VirtualKeyCode::Kanji),
        XKB_KEY_Alt_L => Some(VirtualKeyCode::LAlt),
        XKB_KEY_bracketleft => Some(VirtualKeyCode::LBracket),
        XKB_KEY_Control_L => Some(VirtualKeyCode::LControl),
        XKB_KEY_Meta_L => Some(VirtualKeyCode::LWin),
        XKB_KEY_Shift_L => Some(VirtualKeyCode::LShift),
        XKB_KEY_Super_L => Some(VirtualKeyCode::LWin),
        XKB_KEY_XF86Mail => Some(VirtualKeyCode::Mail),
        XKB_KEY_XF86AudioStop => Some(VirtualKeyCode::MediaStop),
        // => Some(VirtualKeyCode::MediaSelect),
        XKB_KEY_minus => Some(VirtualKeyCode::Minus),
        XKB_KEY_asterisk => Some(VirtualKeyCode::Asterisk),
        XKB_KEY_XF86AudioMute => Some(VirtualKeyCode::Mute),
        XKB_KEY_XF86MyComputer => Some(VirtualKeyCode::MyComputer),
        XKB_KEY_XF86AudioNext => Some(VirtualKeyCode::NextTrack),
        // => Some(VirtualKeyCode::NoConvert),
        XKB_KEY_KP_Separator => Some(VirtualKeyCode::NumpadComma),
        XKB_KEY_KP_Enter => Some(VirtualKeyCode::NumpadEnter),
        XKB_KEY_KP_Equal => Some(VirtualKeyCode::NumpadEquals),
        XKB_KEY_KP_Add => Some(VirtualKeyCode::NumpadAdd),
        XKB_KEY_KP_Subtract => Some(VirtualKeyCode::NumpadSubtract),
        XKB_KEY_KP_Multiply => Some(VirtualKeyCode::NumpadMultiply),
        XKB_KEY_KP_Divide => Some(VirtualKeyCode::NumpadDivide),
        XKB_KEY_KP_Decimal => Some(VirtualKeyCode::NumpadDecimal),
        XKB_KEY_KP_Page_Up => Some(VirtualKeyCode::PageUp),
        XKB_KEY_KP_Page_Down => Some(VirtualKeyCode::PageDown),
        XKB_KEY_KP_Home => Some(VirtualKeyCode::Home),
        XKB_KEY_KP_End => Some(VirtualKeyCode::End),
        XKB_KEY_KP_Left => Some(VirtualKeyCode::Left),
        XKB_KEY_KP_Up => Some(VirtualKeyCode::Up),
        XKB_KEY_KP_Right => Some(VirtualKeyCode::Right),
        XKB_KEY_KP_Down => Some(VirtualKeyCode::Down),
        // => Some(VirtualKeyCode::OEM102),
        XKB_KEY_period => Some(VirtualKeyCode::Period),
        // => Some(VirtualKeyCode::Playpause),
        XKB_KEY_XF86PowerOff => Some(VirtualKeyCode::Power),
        XKB_KEY_XF86AudioPrev => Some(VirtualKeyCode::PrevTrack),
        XKB_KEY_Alt_R => Some(VirtualKeyCode::RAlt),
        XKB_KEY_bracketright => Some(VirtualKeyCode::RBracket),
        XKB_KEY_Control_R => Some(VirtualKeyCode::RControl),
        XKB_KEY_Meta_R => Some(VirtualKeyCode::RWin),
        XKB_KEY_Shift_R => Some(VirtualKeyCode::RShift),
        XKB_KEY_Super_R => Some(VirtualKeyCode::RWin),
        XKB_KEY_semicolon => Some(VirtualKeyCode::Semicolon),
        XKB_KEY_slash => Some(VirtualKeyCode::Slash),
        XKB_KEY_XF86Sleep => Some(VirtualKeyCode::Sleep),
        XKB_KEY_XF86Stop => Some(VirtualKeyCode::Stop),
        XKB_KEY_Tab => Some(VirtualKeyCode::Tab),
        XKB_KEY_ISO_Left_Tab => Some(VirtualKeyCode::Tab),
        XKB_KEY_underscore => Some(VirtualKeyCode::Underline),
        // => Some(VirtualKeyCode::Unlabeled),
        XKB_KEY_XF86AudioLowerVolume => Some(VirtualKeyCode::VolumeDown),
        XKB_KEY_XF86AudioRaiseVolume => Some(VirtualKeyCode::VolumeUp),
        XKB_KEY_XF86WakeUp => Some(VirtualKeyCode::Wake),
        // => Some(VirtualKeyCode::Webback),
        XKB_KEY_XF86Favorites => Some(VirtualKeyCode::WebFavorites),
        // => Some(VirtualKeyCode::WebForward),
        XKB_KEY_XF86HomePage => Some(VirtualKeyCode::WebHome),
        XKB_KEY_XF86Refresh => Some(VirtualKeyCode::WebRefresh),
        // => Some(VirtualKeyCode::WebSearch),
        // => Some(VirtualKeyCode::WebStop),
        XKB_KEY_yen => Some(VirtualKeyCode::Yen),
        XKB_KEY_XF86Copy => Some(VirtualKeyCode::Copy),
        XKB_KEY_XF86Paste => Some(VirtualKeyCode::Paste),
        XKB_KEY_XF86Cut => Some(VirtualKeyCode::Cut),
        // Fallback.
        _ => None,
    }
}
