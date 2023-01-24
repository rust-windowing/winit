use std::{
    char,
    sync::atomic::{AtomicBool, AtomicIsize, Ordering},
};

use windows_sys::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyState, GetKeyboardLayout, GetKeyboardState, MapVirtualKeyA, ToUnicodeEx,
            MAPVK_VK_TO_CHAR, MAPVK_VSC_TO_VK_EX, VIRTUAL_KEY, VK_0, VK_1, VK_2, VK_3, VK_4, VK_5,
            VK_6, VK_7, VK_8, VK_9, VK_A, VK_ADD, VK_APPS, VK_B, VK_BACK, VK_BROWSER_BACK,
            VK_BROWSER_FAVORITES, VK_BROWSER_FORWARD, VK_BROWSER_HOME, VK_BROWSER_REFRESH,
            VK_BROWSER_SEARCH, VK_BROWSER_STOP, VK_C, VK_CAPITAL, VK_CONTROL, VK_CONVERT, VK_D,
            VK_DECIMAL, VK_DELETE, VK_DIVIDE, VK_DOWN, VK_E, VK_END, VK_ESCAPE, VK_F, VK_F1,
            VK_F10, VK_F11, VK_F12, VK_F13, VK_F14, VK_F15, VK_F16, VK_F17, VK_F18, VK_F19, VK_F2,
            VK_F20, VK_F21, VK_F22, VK_F23, VK_F24, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7, VK_F8,
            VK_F9, VK_G, VK_H, VK_HOME, VK_I, VK_INSERT, VK_J, VK_K, VK_KANA, VK_KANJI, VK_L,
            VK_LAUNCH_MAIL, VK_LAUNCH_MEDIA_SELECT, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT,
            VK_LWIN, VK_M, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
            VK_MEDIA_STOP, VK_MENU, VK_MULTIPLY, VK_N, VK_NEXT, VK_NONCONVERT, VK_NUMLOCK,
            VK_NUMPAD0, VK_NUMPAD1, VK_NUMPAD2, VK_NUMPAD3, VK_NUMPAD4, VK_NUMPAD5, VK_NUMPAD6,
            VK_NUMPAD7, VK_NUMPAD8, VK_NUMPAD9, VK_O, VK_OEM_1, VK_OEM_102, VK_OEM_2, VK_OEM_3,
            VK_OEM_4, VK_OEM_5, VK_OEM_6, VK_OEM_7, VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PERIOD,
            VK_OEM_PLUS, VK_P, VK_PAUSE, VK_PRIOR, VK_Q, VK_R, VK_RCONTROL, VK_RETURN, VK_RIGHT,
            VK_RMENU, VK_RSHIFT, VK_RWIN, VK_S, VK_SCROLL, VK_SHIFT, VK_SLEEP, VK_SNAPSHOT,
            VK_SPACE, VK_SUBTRACT, VK_T, VK_TAB, VK_U, VK_UP, VK_V, VK_VOLUME_DOWN, VK_VOLUME_MUTE,
            VK_VOLUME_UP, VK_W, VK_X, VK_Y, VK_Z,
        },
        TextServices::HKL,
    },
};

use crate::event::{ModifiersState, ScanCode, VirtualKeyCode};

use super::util::has_flag;

fn key_pressed(vkey: VIRTUAL_KEY) -> bool {
    unsafe { has_flag(GetKeyState(vkey as i32), 1 << 15) }
}

pub fn get_key_mods() -> ModifiersState {
    let filter_out_altgr = layout_uses_altgr() && key_pressed(VK_RMENU);

    let mut mods = ModifiersState::empty();
    mods.set(ModifiersState::SHIFT, key_pressed(VK_SHIFT));
    mods.set(
        ModifiersState::CTRL,
        key_pressed(VK_CONTROL) && !filter_out_altgr,
    );
    mods.set(
        ModifiersState::ALT,
        key_pressed(VK_MENU) && !filter_out_altgr,
    );
    mods.set(
        ModifiersState::LOGO,
        key_pressed(VK_LWIN) || key_pressed(VK_RWIN),
    );
    mods
}

bitflags! {
    #[derive(Default)]
    pub struct ModifiersStateSide: u32 {
        const LSHIFT = 0b010;
        const RSHIFT = 0b001;

        const LCTRL = 0b010 << 3;
        const RCTRL = 0b001 << 3;

        const LALT = 0b010 << 6;
        const RALT = 0b001 << 6;

        const LLOGO = 0b010 << 9;
        const RLOGO = 0b001 << 9;
    }
}

impl ModifiersStateSide {
    pub fn filter_out_altgr(&self) -> ModifiersStateSide {
        match layout_uses_altgr() && self.contains(Self::RALT) {
            false => *self,
            true => *self & !(Self::LCTRL | Self::RCTRL | Self::LALT | Self::RALT),
        }
    }
}

impl From<ModifiersStateSide> for ModifiersState {
    fn from(side: ModifiersStateSide) -> Self {
        let mut state = ModifiersState::default();
        state.set(
            Self::SHIFT,
            side.intersects(ModifiersStateSide::LSHIFT | ModifiersStateSide::RSHIFT),
        );
        state.set(
            Self::CTRL,
            side.intersects(ModifiersStateSide::LCTRL | ModifiersStateSide::RCTRL),
        );
        state.set(
            Self::ALT,
            side.intersects(ModifiersStateSide::LALT | ModifiersStateSide::RALT),
        );
        state.set(
            Self::LOGO,
            side.intersects(ModifiersStateSide::LLOGO | ModifiersStateSide::RLOGO),
        );
        state
    }
}

pub fn get_pressed_keys() -> impl Iterator<Item = VIRTUAL_KEY> {
    let mut keyboard_state = vec![0u8; 256];
    unsafe { GetKeyboardState(keyboard_state.as_mut_ptr()) };
    keyboard_state
        .into_iter()
        .enumerate()
        .filter(|(_, p)| (*p & (1 << 7)) != 0) // whether or not a key is pressed is communicated via the high-order bit
        .map(|(i, _)| i as u16)
}

unsafe fn get_char(keyboard_state: &[u8; 256], v_key: u32, hkl: HKL) -> Option<char> {
    let mut unicode_bytes = [0u16; 5];
    let len = ToUnicodeEx(
        v_key,
        0,
        keyboard_state.as_ptr(),
        unicode_bytes.as_mut_ptr(),
        unicode_bytes.len() as _,
        0,
        hkl,
    );
    if len >= 1 {
        char::decode_utf16(unicode_bytes.iter().cloned())
            .next()
            .and_then(|c| c.ok())
    } else {
        None
    }
}

/// Figures out if the keyboard layout has an AltGr key instead of an Alt key.
///
/// Unfortunately, the Windows API doesn't give a way for us to conveniently figure that out. So,
/// we use a technique blatantly stolen from [the Firefox source code][source]: iterate over every
/// possible virtual key and compare the `char` output when AltGr is pressed vs when it isn't. If
/// pressing AltGr outputs characters that are different from the standard characters, the layout
/// uses AltGr. Otherwise, it doesn't.
///
/// [source]: https://github.com/mozilla/gecko-dev/blob/265e6721798a455604328ed5262f430cfcc37c2f/widget/windows/KeyboardLayout.cpp#L4356-L4416
fn layout_uses_altgr() -> bool {
    unsafe {
        static ACTIVE_LAYOUT: AtomicIsize = AtomicIsize::new(0);
        static USES_ALTGR: AtomicBool = AtomicBool::new(false);

        let hkl = GetKeyboardLayout(0);
        let old_hkl = ACTIVE_LAYOUT.swap(hkl, Ordering::SeqCst);

        if hkl == old_hkl {
            return USES_ALTGR.load(Ordering::SeqCst);
        }

        let mut keyboard_state_altgr = [0u8; 256];
        // AltGr is an alias for Ctrl+Alt for... some reason. Whatever it is, those are the keypresses
        // we have to emulate to do an AltGr test.
        keyboard_state_altgr[VK_MENU as usize] = 0x80;
        keyboard_state_altgr[VK_CONTROL as usize] = 0x80;

        let keyboard_state_empty = [0u8; 256];

        for v_key in 0..=255 {
            let key_noaltgr = get_char(&keyboard_state_empty, v_key, hkl);
            let key_altgr = get_char(&keyboard_state_altgr, v_key, hkl);
            if let (Some(noaltgr), Some(altgr)) = (key_noaltgr, key_altgr) {
                if noaltgr != altgr {
                    USES_ALTGR.store(true, Ordering::SeqCst);
                    return true;
                }
            }
        }

        USES_ALTGR.store(false, Ordering::SeqCst);
        false
    }
}

pub fn vkey_to_winit_vkey(vkey: VIRTUAL_KEY) -> Option<VirtualKeyCode> {
    // VK_* codes are documented here https://msdn.microsoft.com/en-us/library/windows/desktop/dd375731(v=vs.85).aspx
    match vkey {
        //VK_LBUTTON => Some(VirtualKeyCode::Lbutton),
        //VK_RBUTTON => Some(VirtualKeyCode::Rbutton),
        //VK_CANCEL => Some(VirtualKeyCode::Cancel),
        //VK_MBUTTON => Some(VirtualKeyCode::Mbutton),
        //VK_XBUTTON1 => Some(VirtualKeyCode::Xbutton1),
        //VK_XBUTTON2 => Some(VirtualKeyCode::Xbutton2),
        VK_BACK => Some(VirtualKeyCode::Back),
        VK_TAB => Some(VirtualKeyCode::Tab),
        //VK_CLEAR => Some(VirtualKeyCode::Clear),
        VK_RETURN => Some(VirtualKeyCode::Return),
        VK_LSHIFT => Some(VirtualKeyCode::LShift),
        VK_RSHIFT => Some(VirtualKeyCode::RShift),
        VK_LCONTROL => Some(VirtualKeyCode::LControl),
        VK_RCONTROL => Some(VirtualKeyCode::RControl),
        VK_LMENU => Some(VirtualKeyCode::LAlt),
        VK_RMENU => Some(VirtualKeyCode::RAlt),
        VK_PAUSE => Some(VirtualKeyCode::Pause),
        VK_CAPITAL => Some(VirtualKeyCode::Capital),
        VK_KANA => Some(VirtualKeyCode::Kana),
        //VK_HANGUEL => Some(VirtualKeyCode::Hanguel),
        //VK_HANGUL => Some(VirtualKeyCode::Hangul),
        //VK_JUNJA => Some(VirtualKeyCode::Junja),
        //VK_FINAL => Some(VirtualKeyCode::Final),
        //VK_HANJA => Some(VirtualKeyCode::Hanja),
        VK_KANJI => Some(VirtualKeyCode::Kanji),
        VK_ESCAPE => Some(VirtualKeyCode::Escape),
        VK_CONVERT => Some(VirtualKeyCode::Convert),
        VK_NONCONVERT => Some(VirtualKeyCode::NoConvert),
        //VK_ACCEPT => Some(VirtualKeyCode::Accept),
        //VK_MODECHANGE => Some(VirtualKeyCode::Modechange),
        VK_SPACE => Some(VirtualKeyCode::Space),
        VK_PRIOR => Some(VirtualKeyCode::PageUp),
        VK_NEXT => Some(VirtualKeyCode::PageDown),
        VK_END => Some(VirtualKeyCode::End),
        VK_HOME => Some(VirtualKeyCode::Home),
        VK_LEFT => Some(VirtualKeyCode::Left),
        VK_UP => Some(VirtualKeyCode::Up),
        VK_RIGHT => Some(VirtualKeyCode::Right),
        VK_DOWN => Some(VirtualKeyCode::Down),
        //VK_SELECT => Some(VirtualKeyCode::Select),
        //VK_PRINT => Some(VirtualKeyCode::Print),
        //VK_EXECUTE => Some(VirtualKeyCode::Execute),
        VK_SNAPSHOT => Some(VirtualKeyCode::Snapshot),
        VK_INSERT => Some(VirtualKeyCode::Insert),
        VK_DELETE => Some(VirtualKeyCode::Delete),
        //VK_HELP => Some(VirtualKeyCode::Help),
        VK_0 => Some(VirtualKeyCode::Key0),
        VK_1 => Some(VirtualKeyCode::Key1),
        VK_2 => Some(VirtualKeyCode::Key2),
        VK_3 => Some(VirtualKeyCode::Key3),
        VK_4 => Some(VirtualKeyCode::Key4),
        VK_5 => Some(VirtualKeyCode::Key5),
        VK_6 => Some(VirtualKeyCode::Key6),
        VK_7 => Some(VirtualKeyCode::Key7),
        VK_8 => Some(VirtualKeyCode::Key8),
        VK_9 => Some(VirtualKeyCode::Key9),
        VK_A => Some(VirtualKeyCode::A),
        VK_B => Some(VirtualKeyCode::B),
        VK_C => Some(VirtualKeyCode::C),
        VK_D => Some(VirtualKeyCode::D),
        VK_E => Some(VirtualKeyCode::E),
        VK_F => Some(VirtualKeyCode::F),
        VK_G => Some(VirtualKeyCode::G),
        VK_H => Some(VirtualKeyCode::H),
        VK_I => Some(VirtualKeyCode::I),
        VK_J => Some(VirtualKeyCode::J),
        VK_K => Some(VirtualKeyCode::K),
        VK_L => Some(VirtualKeyCode::L),
        VK_M => Some(VirtualKeyCode::M),
        VK_N => Some(VirtualKeyCode::N),
        VK_O => Some(VirtualKeyCode::O),
        VK_P => Some(VirtualKeyCode::P),
        VK_Q => Some(VirtualKeyCode::Q),
        VK_R => Some(VirtualKeyCode::R),
        VK_S => Some(VirtualKeyCode::S),
        VK_T => Some(VirtualKeyCode::T),
        VK_U => Some(VirtualKeyCode::U),
        VK_V => Some(VirtualKeyCode::V),
        VK_W => Some(VirtualKeyCode::W),
        VK_X => Some(VirtualKeyCode::X),
        VK_Y => Some(VirtualKeyCode::Y),
        VK_Z => Some(VirtualKeyCode::Z),
        VK_LWIN => Some(VirtualKeyCode::LWin),
        VK_RWIN => Some(VirtualKeyCode::RWin),
        VK_APPS => Some(VirtualKeyCode::Apps),
        VK_SLEEP => Some(VirtualKeyCode::Sleep),
        VK_NUMPAD0 => Some(VirtualKeyCode::Numpad0),
        VK_NUMPAD1 => Some(VirtualKeyCode::Numpad1),
        VK_NUMPAD2 => Some(VirtualKeyCode::Numpad2),
        VK_NUMPAD3 => Some(VirtualKeyCode::Numpad3),
        VK_NUMPAD4 => Some(VirtualKeyCode::Numpad4),
        VK_NUMPAD5 => Some(VirtualKeyCode::Numpad5),
        VK_NUMPAD6 => Some(VirtualKeyCode::Numpad6),
        VK_NUMPAD7 => Some(VirtualKeyCode::Numpad7),
        VK_NUMPAD8 => Some(VirtualKeyCode::Numpad8),
        VK_NUMPAD9 => Some(VirtualKeyCode::Numpad9),
        VK_MULTIPLY => Some(VirtualKeyCode::NumpadMultiply),
        VK_ADD => Some(VirtualKeyCode::NumpadAdd),
        //VK_SEPARATOR => Some(VirtualKeyCode::Separator),
        VK_SUBTRACT => Some(VirtualKeyCode::NumpadSubtract),
        VK_DECIMAL => Some(VirtualKeyCode::NumpadDecimal),
        VK_DIVIDE => Some(VirtualKeyCode::NumpadDivide),
        VK_F1 => Some(VirtualKeyCode::F1),
        VK_F2 => Some(VirtualKeyCode::F2),
        VK_F3 => Some(VirtualKeyCode::F3),
        VK_F4 => Some(VirtualKeyCode::F4),
        VK_F5 => Some(VirtualKeyCode::F5),
        VK_F6 => Some(VirtualKeyCode::F6),
        VK_F7 => Some(VirtualKeyCode::F7),
        VK_F8 => Some(VirtualKeyCode::F8),
        VK_F9 => Some(VirtualKeyCode::F9),
        VK_F10 => Some(VirtualKeyCode::F10),
        VK_F11 => Some(VirtualKeyCode::F11),
        VK_F12 => Some(VirtualKeyCode::F12),
        VK_F13 => Some(VirtualKeyCode::F13),
        VK_F14 => Some(VirtualKeyCode::F14),
        VK_F15 => Some(VirtualKeyCode::F15),
        VK_F16 => Some(VirtualKeyCode::F16),
        VK_F17 => Some(VirtualKeyCode::F17),
        VK_F18 => Some(VirtualKeyCode::F18),
        VK_F19 => Some(VirtualKeyCode::F19),
        VK_F20 => Some(VirtualKeyCode::F20),
        VK_F21 => Some(VirtualKeyCode::F21),
        VK_F22 => Some(VirtualKeyCode::F22),
        VK_F23 => Some(VirtualKeyCode::F23),
        VK_F24 => Some(VirtualKeyCode::F24),
        VK_NUMLOCK => Some(VirtualKeyCode::Numlock),
        VK_SCROLL => Some(VirtualKeyCode::Scroll),
        VK_BROWSER_BACK => Some(VirtualKeyCode::NavigateBackward),
        VK_BROWSER_FORWARD => Some(VirtualKeyCode::NavigateForward),
        VK_BROWSER_REFRESH => Some(VirtualKeyCode::WebRefresh),
        VK_BROWSER_STOP => Some(VirtualKeyCode::WebStop),
        VK_BROWSER_SEARCH => Some(VirtualKeyCode::WebSearch),
        VK_BROWSER_FAVORITES => Some(VirtualKeyCode::WebFavorites),
        VK_BROWSER_HOME => Some(VirtualKeyCode::WebHome),
        VK_VOLUME_MUTE => Some(VirtualKeyCode::Mute),
        VK_VOLUME_DOWN => Some(VirtualKeyCode::VolumeDown),
        VK_VOLUME_UP => Some(VirtualKeyCode::VolumeUp),
        VK_MEDIA_NEXT_TRACK => Some(VirtualKeyCode::NextTrack),
        VK_MEDIA_PREV_TRACK => Some(VirtualKeyCode::PrevTrack),
        VK_MEDIA_STOP => Some(VirtualKeyCode::MediaStop),
        VK_MEDIA_PLAY_PAUSE => Some(VirtualKeyCode::PlayPause),
        VK_LAUNCH_MAIL => Some(VirtualKeyCode::Mail),
        VK_LAUNCH_MEDIA_SELECT => Some(VirtualKeyCode::MediaSelect),
        /*VK_LAUNCH_APP1 => Some(VirtualKeyCode::Launch_app1),
        VK_LAUNCH_APP2 => Some(VirtualKeyCode::Launch_app2),*/
        VK_OEM_PLUS => Some(VirtualKeyCode::Equals),
        VK_OEM_COMMA => Some(VirtualKeyCode::Comma),
        VK_OEM_MINUS => Some(VirtualKeyCode::Minus),
        VK_OEM_PERIOD => Some(VirtualKeyCode::Period),
        VK_OEM_1 => map_text_keys(vkey),
        VK_OEM_2 => map_text_keys(vkey),
        VK_OEM_3 => map_text_keys(vkey),
        VK_OEM_4 => map_text_keys(vkey),
        VK_OEM_5 => map_text_keys(vkey),
        VK_OEM_6 => map_text_keys(vkey),
        VK_OEM_7 => map_text_keys(vkey),
        /* VK_OEM_8 => Some(VirtualKeyCode::Oem_8), */
        VK_OEM_102 => Some(VirtualKeyCode::OEM102),
        /*VK_PROCESSKEY => Some(VirtualKeyCode::Processkey),
        VK_PACKET => Some(VirtualKeyCode::Packet),
        VK_ATTN => Some(VirtualKeyCode::Attn),
        VK_CRSEL => Some(VirtualKeyCode::Crsel),
        VK_EXSEL => Some(VirtualKeyCode::Exsel),
        VK_EREOF => Some(VirtualKeyCode::Ereof),
        VK_PLAY => Some(VirtualKeyCode::Play),
        VK_ZOOM => Some(VirtualKeyCode::Zoom),
        VK_NONAME => Some(VirtualKeyCode::Noname),
        VK_PA1 => Some(VirtualKeyCode::Pa1),
        VK_OEM_CLEAR => Some(VirtualKeyCode::Oem_clear),*/
        _ => None,
    }
}

pub fn handle_extended_keys(
    vkey: VIRTUAL_KEY,
    mut scancode: u32,
    extended: bool,
) -> Option<(VIRTUAL_KEY, u32)> {
    // Welcome to hell https://blog.molecular-matters.com/2011/09/05/properly-handling-keyboard-input/
    scancode |= if extended { 0xE000 } else { 0x0000 };
    let vkey = match vkey {
        VK_SHIFT => (unsafe { MapVirtualKeyA(scancode, MAPVK_VSC_TO_VK_EX) } as u16),
        VK_CONTROL => {
            if extended {
                VK_RCONTROL
            } else {
                VK_LCONTROL
            }
        }
        VK_MENU => {
            if extended {
                VK_RMENU
            } else {
                VK_LMENU
            }
        }
        _ => {
            match scancode {
                // When VK_PAUSE is pressed it emits a LeftControl + NumLock scancode event sequence, but reports VK_PAUSE
                // as the virtual key on both events, or VK_PAUSE on the first event or 0xFF when using raw input.
                // Don't emit anything for the LeftControl event in the pair...
                0xE01D if vkey == VK_PAUSE => return None,
                // ...and emit the Pause event for the second event in the pair.
                0x45 if vkey == VK_PAUSE || vkey == 0xFF => {
                    scancode = 0xE059;
                    VK_PAUSE
                }
                // VK_PAUSE has an incorrect vkey value when used with modifiers. VK_PAUSE also reports a different
                // scancode when used with modifiers than when used without
                0xE046 => {
                    scancode = 0xE059;
                    VK_PAUSE
                }
                // VK_SCROLL has an incorrect vkey value when used with modifiers.
                0x46 => VK_SCROLL,
                _ => vkey,
            }
        }
    };
    Some((vkey, scancode))
}

pub fn process_key_params(
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<(ScanCode, Option<VirtualKeyCode>)> {
    let scancode = ((lparam >> 16) & 0xff) as u32;
    let extended = (lparam & 0x01000000) != 0;
    handle_extended_keys(wparam as u16, scancode, extended)
        .map(|(vkey, scancode)| (scancode, vkey_to_winit_vkey(vkey)))
}

// This is needed as windows doesn't properly distinguish
// some virtual key codes for different keyboard layouts
fn map_text_keys(win_virtual_key: VIRTUAL_KEY) -> Option<VirtualKeyCode> {
    let char_key = unsafe { MapVirtualKeyA(win_virtual_key as u32, MAPVK_VK_TO_CHAR) } & 0x7FFF;
    match char::from_u32(char_key) {
        Some(';') => Some(VirtualKeyCode::Semicolon),
        Some('/') => Some(VirtualKeyCode::Slash),
        Some('`') => Some(VirtualKeyCode::Grave),
        Some('[') => Some(VirtualKeyCode::LBracket),
        Some(']') => Some(VirtualKeyCode::RBracket),
        Some('\'') => Some(VirtualKeyCode::Apostrophe),
        Some('\\') => Some(VirtualKeyCode::Backslash),
        _ => None,
    }
}
