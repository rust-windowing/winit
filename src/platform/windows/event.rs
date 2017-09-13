use events::VirtualKeyCode;
use events::ModifiersState;
use winapi;
use user32;
use ScanCode;
use std::char;

const MAPVK_VK_TO_CHAR: u32 = 2;
const MAPVK_VSC_TO_VK_EX: u32 = 3;

pub fn get_key_mods() -> ModifiersState {
    let mut mods = ModifiersState::default();
    unsafe {
        if user32::GetKeyState(winapi::VK_SHIFT) & (1 << 15) == (1 << 15) {
            mods.shift = true;
        }
        if user32::GetKeyState(winapi::VK_CONTROL) & (1 << 15) == (1 << 15) {
            mods.ctrl = true;
        }
        if user32::GetKeyState(winapi::VK_MENU) & (1 << 15) == (1 << 15) {
            mods.alt = true;
        }
        if (user32::GetKeyState(winapi::VK_LWIN) | user32::GetKeyState(winapi::VK_RWIN)) & (1 << 15) == (1 << 15) {
            mods.logo = true;
        }
    }
    mods
}

pub fn vkeycode_to_element(wparam: winapi::WPARAM, lparam: winapi::LPARAM) -> (ScanCode, Option<VirtualKeyCode>) {
    let scancode = ((lparam >> 16) & 0xff) as u32;
    let extended = (lparam & 0x01000000) != 0;
    let vk = match wparam as i32 {
        winapi::VK_SHIFT => unsafe { user32::MapVirtualKeyA(scancode, MAPVK_VSC_TO_VK_EX) as i32 },
        winapi::VK_CONTROL => if extended { winapi::VK_RCONTROL } else { winapi::VK_LCONTROL },
        winapi::VK_MENU => if extended { winapi::VK_RMENU } else { winapi::VK_LMENU },
        other => other
    };

    // VK_* codes are documented here https://msdn.microsoft.com/en-us/library/windows/desktop/dd375731(v=vs.85).aspx
    (scancode, match vk {
        //winapi::VK_LBUTTON => Some(VirtualKeyCode::Lbutton),
        //winapi::VK_RBUTTON => Some(VirtualKeyCode::Rbutton),
        //winapi::VK_CANCEL => Some(VirtualKeyCode::Cancel),
        //winapi::VK_MBUTTON => Some(VirtualKeyCode::Mbutton),
        //winapi::VK_XBUTTON1 => Some(VirtualKeyCode::Xbutton1),
        //winapi::VK_XBUTTON2 => Some(VirtualKeyCode::Xbutton2),
        winapi::VK_BACK => Some(VirtualKeyCode::Back),
        winapi::VK_TAB => Some(VirtualKeyCode::Tab),
        //winapi::VK_CLEAR => Some(VirtualKeyCode::Clear),
        winapi::VK_RETURN => Some(VirtualKeyCode::Return),
        winapi::VK_LSHIFT => Some(VirtualKeyCode::LShift),
        winapi::VK_RSHIFT => Some(VirtualKeyCode::RShift),
        winapi::VK_LCONTROL => Some(VirtualKeyCode::LControl),
        winapi::VK_RCONTROL => Some(VirtualKeyCode::RControl),
        winapi::VK_LMENU => Some(VirtualKeyCode::LMenu),
        winapi::VK_RMENU => Some(VirtualKeyCode::RMenu),
        winapi::VK_PAUSE => Some(VirtualKeyCode::Pause),
        winapi::VK_CAPITAL => Some(VirtualKeyCode::Capital),
        winapi::VK_KANA => Some(VirtualKeyCode::Kana),
        //winapi::VK_HANGUEL => Some(VirtualKeyCode::Hanguel),
        //winapi::VK_HANGUL => Some(VirtualKeyCode::Hangul),
        //winapi::VK_JUNJA => Some(VirtualKeyCode::Junja),
        //winapi::VK_FINAL => Some(VirtualKeyCode::Final),
        //winapi::VK_HANJA => Some(VirtualKeyCode::Hanja),
        winapi::VK_KANJI => Some(VirtualKeyCode::Kanji),
        winapi::VK_ESCAPE => Some(VirtualKeyCode::Escape),
        winapi::VK_CONVERT => Some(VirtualKeyCode::Convert),
        winapi::VK_NONCONVERT => Some(VirtualKeyCode::NoConvert),
        //winapi::VK_ACCEPT => Some(VirtualKeyCode::Accept),
        //winapi::VK_MODECHANGE => Some(VirtualKeyCode::Modechange),
        winapi::VK_SPACE => Some(VirtualKeyCode::Space),
        winapi::VK_PRIOR => Some(VirtualKeyCode::PageUp),
        winapi::VK_NEXT => Some(VirtualKeyCode::PageDown),
        winapi::VK_END => Some(VirtualKeyCode::End),
        winapi::VK_HOME => Some(VirtualKeyCode::Home),
        winapi::VK_LEFT => Some(VirtualKeyCode::Left),
        winapi::VK_UP => Some(VirtualKeyCode::Up),
        winapi::VK_RIGHT => Some(VirtualKeyCode::Right),
        winapi::VK_DOWN => Some(VirtualKeyCode::Down),
        //winapi::VK_SELECT => Some(VirtualKeyCode::Select),
        //winapi::VK_PRINT => Some(VirtualKeyCode::Print),
        //winapi::VK_EXECUTE => Some(VirtualKeyCode::Execute),
        winapi::VK_SNAPSHOT => Some(VirtualKeyCode::Snapshot),
        winapi::VK_INSERT => Some(VirtualKeyCode::Insert),
        winapi::VK_DELETE => Some(VirtualKeyCode::Delete),
        //winapi::VK_HELP => Some(VirtualKeyCode::Help),
        0x30 => Some(VirtualKeyCode::Key0),
        0x31 => Some(VirtualKeyCode::Key1),
        0x32 => Some(VirtualKeyCode::Key2),
        0x33 => Some(VirtualKeyCode::Key3),
        0x34 => Some(VirtualKeyCode::Key4),
        0x35 => Some(VirtualKeyCode::Key5),
        0x36 => Some(VirtualKeyCode::Key6),
        0x37 => Some(VirtualKeyCode::Key7),
        0x38 => Some(VirtualKeyCode::Key8),
        0x39 => Some(VirtualKeyCode::Key9),
        0x41 => Some(VirtualKeyCode::A),
        0x42 => Some(VirtualKeyCode::B),
        0x43 => Some(VirtualKeyCode::C),
        0x44 => Some(VirtualKeyCode::D),
        0x45 => Some(VirtualKeyCode::E),
        0x46 => Some(VirtualKeyCode::F),
        0x47 => Some(VirtualKeyCode::G),
        0x48 => Some(VirtualKeyCode::H),
        0x49 => Some(VirtualKeyCode::I),
        0x4A => Some(VirtualKeyCode::J),
        0x4B => Some(VirtualKeyCode::K),
        0x4C => Some(VirtualKeyCode::L),
        0x4D => Some(VirtualKeyCode::M),
        0x4E => Some(VirtualKeyCode::N),
        0x4F => Some(VirtualKeyCode::O),
        0x50 => Some(VirtualKeyCode::P),
        0x51 => Some(VirtualKeyCode::Q),
        0x52 => Some(VirtualKeyCode::R),
        0x53 => Some(VirtualKeyCode::S),
        0x54 => Some(VirtualKeyCode::T),
        0x55 => Some(VirtualKeyCode::U),
        0x56 => Some(VirtualKeyCode::V),
        0x57 => Some(VirtualKeyCode::W),
        0x58 => Some(VirtualKeyCode::X),
        0x59 => Some(VirtualKeyCode::Y),
        0x5A => Some(VirtualKeyCode::Z),
        //winapi::VK_LWIN => Some(VirtualKeyCode::Lwin),
        //winapi::VK_RWIN => Some(VirtualKeyCode::Rwin),
        winapi::VK_APPS => Some(VirtualKeyCode::Apps),
        winapi::VK_SLEEP => Some(VirtualKeyCode::Sleep),
        winapi::VK_NUMPAD0 => Some(VirtualKeyCode::Numpad0),
        winapi::VK_NUMPAD1 => Some(VirtualKeyCode::Numpad1),
        winapi::VK_NUMPAD2 => Some(VirtualKeyCode::Numpad2),
        winapi::VK_NUMPAD3 => Some(VirtualKeyCode::Numpad3),
        winapi::VK_NUMPAD4 => Some(VirtualKeyCode::Numpad4),
        winapi::VK_NUMPAD5 => Some(VirtualKeyCode::Numpad5),
        winapi::VK_NUMPAD6 => Some(VirtualKeyCode::Numpad6),
        winapi::VK_NUMPAD7 => Some(VirtualKeyCode::Numpad7),
        winapi::VK_NUMPAD8 => Some(VirtualKeyCode::Numpad8),
        winapi::VK_NUMPAD9 => Some(VirtualKeyCode::Numpad9),
        winapi::VK_MULTIPLY => Some(VirtualKeyCode::Multiply),
        winapi::VK_ADD => Some(VirtualKeyCode::Add),
        //winapi::VK_SEPARATOR => Some(VirtualKeyCode::Separator),
        winapi::VK_SUBTRACT => Some(VirtualKeyCode::Subtract),
        winapi::VK_DECIMAL => Some(VirtualKeyCode::Decimal),
        winapi::VK_DIVIDE => Some(VirtualKeyCode::Divide),
        winapi::VK_F1 => Some(VirtualKeyCode::F1),
        winapi::VK_F2 => Some(VirtualKeyCode::F2),
        winapi::VK_F3 => Some(VirtualKeyCode::F3),
        winapi::VK_F4 => Some(VirtualKeyCode::F4),
        winapi::VK_F5 => Some(VirtualKeyCode::F5),
        winapi::VK_F6 => Some(VirtualKeyCode::F6),
        winapi::VK_F7 => Some(VirtualKeyCode::F7),
        winapi::VK_F8 => Some(VirtualKeyCode::F8),
        winapi::VK_F9 => Some(VirtualKeyCode::F9),
        winapi::VK_F10 => Some(VirtualKeyCode::F10),
        winapi::VK_F11 => Some(VirtualKeyCode::F11),
        winapi::VK_F12 => Some(VirtualKeyCode::F12),
        winapi::VK_F13 => Some(VirtualKeyCode::F13),
        winapi::VK_F14 => Some(VirtualKeyCode::F14),
        winapi::VK_F15 => Some(VirtualKeyCode::F15),
        /*winapi::VK_F16 => Some(VirtualKeyCode::F16),
        winapi::VK_F17 => Some(VirtualKeyCode::F17),
        winapi::VK_F18 => Some(VirtualKeyCode::F18),
        winapi::VK_F19 => Some(VirtualKeyCode::F19),
        winapi::VK_F20 => Some(VirtualKeyCode::F20),
        winapi::VK_F21 => Some(VirtualKeyCode::F21),
        winapi::VK_F22 => Some(VirtualKeyCode::F22),
        winapi::VK_F23 => Some(VirtualKeyCode::F23),
        winapi::VK_F24 => Some(VirtualKeyCode::F24),*/
        winapi::VK_NUMLOCK => Some(VirtualKeyCode::Numlock),
        winapi::VK_SCROLL => Some(VirtualKeyCode::Scroll),
        winapi::VK_BROWSER_BACK => Some(VirtualKeyCode::NavigateBackward),
        winapi::VK_BROWSER_FORWARD => Some(VirtualKeyCode::NavigateForward),
        winapi::VK_BROWSER_REFRESH => Some(VirtualKeyCode::WebRefresh),
        winapi::VK_BROWSER_STOP => Some(VirtualKeyCode::WebStop),
        winapi::VK_BROWSER_SEARCH => Some(VirtualKeyCode::WebSearch),
        winapi::VK_BROWSER_FAVORITES => Some(VirtualKeyCode::WebFavorites),
        winapi::VK_BROWSER_HOME => Some(VirtualKeyCode::WebHome),
        winapi::VK_VOLUME_MUTE => Some(VirtualKeyCode::Mute),
        winapi::VK_VOLUME_DOWN => Some(VirtualKeyCode::VolumeDown),
        winapi::VK_VOLUME_UP => Some(VirtualKeyCode::VolumeUp),
        winapi::VK_MEDIA_NEXT_TRACK => Some(VirtualKeyCode::NextTrack),
        winapi::VK_MEDIA_PREV_TRACK => Some(VirtualKeyCode::PrevTrack),
        winapi::VK_MEDIA_STOP => Some(VirtualKeyCode::MediaStop),
        winapi::VK_MEDIA_PLAY_PAUSE => Some(VirtualKeyCode::PlayPause),
        winapi::VK_LAUNCH_MAIL => Some(VirtualKeyCode::Mail),
        winapi::VK_LAUNCH_MEDIA_SELECT => Some(VirtualKeyCode::MediaSelect),
        /*winapi::VK_LAUNCH_APP1 => Some(VirtualKeyCode::Launch_app1),
        winapi::VK_LAUNCH_APP2 => Some(VirtualKeyCode::Launch_app2),*/
        winapi::VK_OEM_PLUS => Some(VirtualKeyCode::Equals),
        winapi::VK_OEM_COMMA => Some(VirtualKeyCode::Comma),
        winapi::VK_OEM_MINUS => Some(VirtualKeyCode::Minus),
        winapi::VK_OEM_PERIOD => Some(VirtualKeyCode::Period),
        winapi::VK_OEM_1 => map_text_keys(vk),
        winapi::VK_OEM_2 => map_text_keys(vk),
        winapi::VK_OEM_3 => map_text_keys(vk),
        winapi::VK_OEM_4 => map_text_keys(vk),
        winapi::VK_OEM_5 => map_text_keys(vk),
        winapi::VK_OEM_6 => map_text_keys(vk),
        winapi::VK_OEM_7 => map_text_keys(vk),
        /*winapi::VK_OEM_8 => Some(VirtualKeyCode::Oem_8), */
        winapi::VK_OEM_102 => Some(VirtualKeyCode::OEM102),
        /*winapi::VK_PROCESSKEY => Some(VirtualKeyCode::Processkey),
        winapi::VK_PACKET => Some(VirtualKeyCode::Packet),
        winapi::VK_ATTN => Some(VirtualKeyCode::Attn),
        winapi::VK_CRSEL => Some(VirtualKeyCode::Crsel),
        winapi::VK_EXSEL => Some(VirtualKeyCode::Exsel),
        winapi::VK_EREOF => Some(VirtualKeyCode::Ereof),
        winapi::VK_PLAY => Some(VirtualKeyCode::Play),
        winapi::VK_ZOOM => Some(VirtualKeyCode::Zoom),
        winapi::VK_NONAME => Some(VirtualKeyCode::Noname),
        winapi::VK_PA1 => Some(VirtualKeyCode::Pa1),
        winapi::VK_OEM_CLEAR => Some(VirtualKeyCode::Oem_clear),*/
        _ => None
    })
}

// This is needed as windows doesn't properly distinguish
// some virtual key codes for different keyboard layouts
fn map_text_keys(win_virtual_key: i32) -> Option<VirtualKeyCode> {
    let char_key = unsafe { user32::MapVirtualKeyA(win_virtual_key as u32, MAPVK_VK_TO_CHAR) } & 0x7FFF;
    match char::from_u32(char_key) {
        Some(';') => Some(VirtualKeyCode::Semicolon),
        Some('/') => Some(VirtualKeyCode::Slash),
        Some('`') => Some(VirtualKeyCode::Grave),
        Some('[') => Some(VirtualKeyCode::LBracket),
        Some(']') => Some(VirtualKeyCode::RBracket),
        Some('\'') => Some(VirtualKeyCode::Apostrophe),
        Some('\\') => Some(VirtualKeyCode::Backslash),
        _ => None
    }
}
