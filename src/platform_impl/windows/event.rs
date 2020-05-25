use std::{
    char,
    os::raw::c_int,
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};

use crate::{
    dpi::{PhysicalSize, PhysicalPosition},
    event::{LogicalKey, ModifiersState, Force, PointerTiltEvent, PointerButton, WindowEvent, PointerId, PointerButtonEvent},
};
use std::collections::hash_map::{HashMap, Entry};

use winapi::{
    shared::{
        windef::HWND,
        minwindef::{HKL, HKL__, LPARAM, UINT, WPARAM, DWORD},
    },
    um::{winuser, winreg},
};

fn key_pressed(vkey: c_int) -> bool {
    unsafe { (winuser::GetKeyState(vkey) & (1 << 15)) == (1 << 15) }
}

pub fn get_key_mods() -> ModifiersState {
    let filter_out_altgr = layout_uses_altgr() && key_pressed(winuser::VK_RMENU);

    let mut mods = ModifiersState::empty();
    mods.set(ModifiersState::SHIFT, key_pressed(winuser::VK_SHIFT));
    mods.set(
        ModifiersState::CTRL,
        key_pressed(winuser::VK_CONTROL) && !filter_out_altgr,
    );
    mods.set(
        ModifiersState::ALT,
        key_pressed(winuser::VK_MENU) && !filter_out_altgr,
    );
    mods.set(
        ModifiersState::LOGO,
        key_pressed(winuser::VK_LWIN) || key_pressed(winuser::VK_RWIN),
    );
    mods
}

bitflags! {
    #[derive(Default)]
    pub struct ModifiersStateSide: u32 {
        const LSHIFT = 0b010 << 0;
        const RSHIFT = 0b001 << 0;

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

pub fn get_pressed_keys() -> impl Iterator<Item = c_int> {
    let mut keyboard_state = vec![0u8; 256];
    unsafe { winuser::GetKeyboardState(keyboard_state.as_mut_ptr()) };
    keyboard_state
        .into_iter()
        .enumerate()
        .filter(|(_, p)| (*p & (1 << 7)) != 0) // whether or not a key is pressed is communicated via the high-order bit
        .map(|(i, _)| i as c_int)
}

unsafe fn get_char(keyboard_state: &[u8; 256], v_key: u32, hkl: HKL) -> Option<char> {
    let mut unicode_bytes = [0u16; 5];
    let len = winuser::ToUnicodeEx(
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
        static ACTIVE_LAYOUT: AtomicPtr<HKL__> = AtomicPtr::new(ptr::null_mut());
        static USES_ALTGR: AtomicBool = AtomicBool::new(false);

        let hkl = winuser::GetKeyboardLayout(0);
        let old_hkl = ACTIVE_LAYOUT.swap(hkl, Ordering::SeqCst);

        if hkl == old_hkl {
            return USES_ALTGR.load(Ordering::SeqCst);
        }

        let mut keyboard_state_altgr = [0u8; 256];
        // AltGr is an alias for Ctrl+Alt for... some reason. Whatever it is, those are the keypresses
        // we have to emulate to do an AltGr test.
        keyboard_state_altgr[winuser::VK_MENU as usize] = 0x80;
        keyboard_state_altgr[winuser::VK_CONTROL as usize] = 0x80;

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

pub fn vkey_to_winit_vkey(vkey: c_int) -> Option<LogicalKey> {
    // VK_* codes are documented here https://msdn.microsoft.com/en-us/library/windows/desktop/dd375731(v=vs.85).aspx
    match vkey {
        //winuser::VK_LBUTTON => Some(LogicalKey::Lbutton),
        //winuser::VK_RBUTTON => Some(LogicalKey::Rbutton),
        //winuser::VK_CANCEL => Some(LogicalKey::Cancel),
        //winuser::VK_MBUTTON => Some(LogicalKey::Mbutton),
        //winuser::VK_XBUTTON1 => Some(LogicalKey::Xbutton1),
        //winuser::VK_XBUTTON2 => Some(LogicalKey::Xbutton2),
        winuser::VK_BACK => Some(LogicalKey::Back),
        winuser::VK_TAB => Some(LogicalKey::Tab),
        //winuser::VK_CLEAR => Some(LogicalKey::Clear),
        winuser::VK_RETURN => Some(LogicalKey::Return),
        winuser::VK_LSHIFT => Some(LogicalKey::LShift),
        winuser::VK_RSHIFT => Some(LogicalKey::RShift),
        winuser::VK_LCONTROL => Some(LogicalKey::LControl),
        winuser::VK_RCONTROL => Some(LogicalKey::RControl),
        winuser::VK_LMENU => Some(LogicalKey::LAlt),
        winuser::VK_RMENU => Some(LogicalKey::RAlt),
        winuser::VK_PAUSE => Some(LogicalKey::Pause),
        winuser::VK_CAPITAL => Some(LogicalKey::Capital),
        winuser::VK_KANA => Some(LogicalKey::Kana),
        //winuser::VK_HANGUEL => Some(LogicalKey::Hanguel),
        //winuser::VK_HANGUL => Some(LogicalKey::Hangul),
        //winuser::VK_JUNJA => Some(LogicalKey::Junja),
        //winuser::VK_FINAL => Some(LogicalKey::Final),
        //winuser::VK_HANJA => Some(LogicalKey::Hanja),
        winuser::VK_KANJI => Some(LogicalKey::Kanji),
        winuser::VK_ESCAPE => Some(LogicalKey::Escape),
        winuser::VK_CONVERT => Some(LogicalKey::Convert),
        winuser::VK_NONCONVERT => Some(LogicalKey::NoConvert),
        //winuser::VK_ACCEPT => Some(LogicalKey::Accept),
        //winuser::VK_MODECHANGE => Some(LogicalKey::Modechange),
        winuser::VK_SPACE => Some(LogicalKey::Space),
        winuser::VK_PRIOR => Some(LogicalKey::PageUp),
        winuser::VK_NEXT => Some(LogicalKey::PageDown),
        winuser::VK_END => Some(LogicalKey::End),
        winuser::VK_HOME => Some(LogicalKey::Home),
        winuser::VK_LEFT => Some(LogicalKey::Left),
        winuser::VK_UP => Some(LogicalKey::Up),
        winuser::VK_RIGHT => Some(LogicalKey::Right),
        winuser::VK_DOWN => Some(LogicalKey::Down),
        //winuser::VK_SELECT => Some(LogicalKey::Select),
        //winuser::VK_PRINT => Some(LogicalKey::Print),
        //winuser::VK_EXECUTE => Some(LogicalKey::Execute),
        winuser::VK_SNAPSHOT => Some(LogicalKey::Snapshot),
        winuser::VK_INSERT => Some(LogicalKey::Insert),
        winuser::VK_DELETE => Some(LogicalKey::Delete),
        //winuser::VK_HELP => Some(LogicalKey::Help),
        0x30 => Some(LogicalKey::Key0),
        0x31 => Some(LogicalKey::Key1),
        0x32 => Some(LogicalKey::Key2),
        0x33 => Some(LogicalKey::Key3),
        0x34 => Some(LogicalKey::Key4),
        0x35 => Some(LogicalKey::Key5),
        0x36 => Some(LogicalKey::Key6),
        0x37 => Some(LogicalKey::Key7),
        0x38 => Some(LogicalKey::Key8),
        0x39 => Some(LogicalKey::Key9),
        0x41 => Some(LogicalKey::A),
        0x42 => Some(LogicalKey::B),
        0x43 => Some(LogicalKey::C),
        0x44 => Some(LogicalKey::D),
        0x45 => Some(LogicalKey::E),
        0x46 => Some(LogicalKey::F),
        0x47 => Some(LogicalKey::G),
        0x48 => Some(LogicalKey::H),
        0x49 => Some(LogicalKey::I),
        0x4A => Some(LogicalKey::J),
        0x4B => Some(LogicalKey::K),
        0x4C => Some(LogicalKey::L),
        0x4D => Some(LogicalKey::M),
        0x4E => Some(LogicalKey::N),
        0x4F => Some(LogicalKey::O),
        0x50 => Some(LogicalKey::P),
        0x51 => Some(LogicalKey::Q),
        0x52 => Some(LogicalKey::R),
        0x53 => Some(LogicalKey::S),
        0x54 => Some(LogicalKey::T),
        0x55 => Some(LogicalKey::U),
        0x56 => Some(LogicalKey::V),
        0x57 => Some(LogicalKey::W),
        0x58 => Some(LogicalKey::X),
        0x59 => Some(LogicalKey::Y),
        0x5A => Some(LogicalKey::Z),
        winuser::VK_LWIN => Some(LogicalKey::LWin),
        winuser::VK_RWIN => Some(LogicalKey::RWin),
        winuser::VK_APPS => Some(LogicalKey::Apps),
        winuser::VK_SLEEP => Some(LogicalKey::Sleep),
        winuser::VK_NUMPAD0 => Some(LogicalKey::Numpad0),
        winuser::VK_NUMPAD1 => Some(LogicalKey::Numpad1),
        winuser::VK_NUMPAD2 => Some(LogicalKey::Numpad2),
        winuser::VK_NUMPAD3 => Some(LogicalKey::Numpad3),
        winuser::VK_NUMPAD4 => Some(LogicalKey::Numpad4),
        winuser::VK_NUMPAD5 => Some(LogicalKey::Numpad5),
        winuser::VK_NUMPAD6 => Some(LogicalKey::Numpad6),
        winuser::VK_NUMPAD7 => Some(LogicalKey::Numpad7),
        winuser::VK_NUMPAD8 => Some(LogicalKey::Numpad8),
        winuser::VK_NUMPAD9 => Some(LogicalKey::Numpad9),
        winuser::VK_MULTIPLY => Some(LogicalKey::Multiply),
        winuser::VK_ADD => Some(LogicalKey::Add),
        //winuser::VK_SEPARATOR => Some(LogicalKey::Separator),
        winuser::VK_SUBTRACT => Some(LogicalKey::Subtract),
        winuser::VK_DECIMAL => Some(LogicalKey::Decimal),
        winuser::VK_DIVIDE => Some(LogicalKey::Divide),
        winuser::VK_F1 => Some(LogicalKey::F1),
        winuser::VK_F2 => Some(LogicalKey::F2),
        winuser::VK_F3 => Some(LogicalKey::F3),
        winuser::VK_F4 => Some(LogicalKey::F4),
        winuser::VK_F5 => Some(LogicalKey::F5),
        winuser::VK_F6 => Some(LogicalKey::F6),
        winuser::VK_F7 => Some(LogicalKey::F7),
        winuser::VK_F8 => Some(LogicalKey::F8),
        winuser::VK_F9 => Some(LogicalKey::F9),
        winuser::VK_F10 => Some(LogicalKey::F10),
        winuser::VK_F11 => Some(LogicalKey::F11),
        winuser::VK_F12 => Some(LogicalKey::F12),
        winuser::VK_F13 => Some(LogicalKey::F13),
        winuser::VK_F14 => Some(LogicalKey::F14),
        winuser::VK_F15 => Some(LogicalKey::F15),
        winuser::VK_F16 => Some(LogicalKey::F16),
        winuser::VK_F17 => Some(LogicalKey::F17),
        winuser::VK_F18 => Some(LogicalKey::F18),
        winuser::VK_F19 => Some(LogicalKey::F19),
        winuser::VK_F20 => Some(LogicalKey::F20),
        winuser::VK_F21 => Some(LogicalKey::F21),
        winuser::VK_F22 => Some(LogicalKey::F22),
        winuser::VK_F23 => Some(LogicalKey::F23),
        winuser::VK_F24 => Some(LogicalKey::F24),
        winuser::VK_NUMLOCK => Some(LogicalKey::Numlock),
        winuser::VK_SCROLL => Some(LogicalKey::Scroll),
        winuser::VK_BROWSER_BACK => Some(LogicalKey::NavigateBackward),
        winuser::VK_BROWSER_FORWARD => Some(LogicalKey::NavigateForward),
        winuser::VK_BROWSER_REFRESH => Some(LogicalKey::WebRefresh),
        winuser::VK_BROWSER_STOP => Some(LogicalKey::WebStop),
        winuser::VK_BROWSER_SEARCH => Some(LogicalKey::WebSearch),
        winuser::VK_BROWSER_FAVORITES => Some(LogicalKey::WebFavorites),
        winuser::VK_BROWSER_HOME => Some(LogicalKey::WebHome),
        winuser::VK_VOLUME_MUTE => Some(LogicalKey::Mute),
        winuser::VK_VOLUME_DOWN => Some(LogicalKey::VolumeDown),
        winuser::VK_VOLUME_UP => Some(LogicalKey::VolumeUp),
        winuser::VK_MEDIA_NEXT_TRACK => Some(LogicalKey::NextTrack),
        winuser::VK_MEDIA_PREV_TRACK => Some(LogicalKey::PrevTrack),
        winuser::VK_MEDIA_STOP => Some(LogicalKey::MediaStop),
        winuser::VK_MEDIA_PLAY_PAUSE => Some(LogicalKey::PlayPause),
        winuser::VK_LAUNCH_MAIL => Some(LogicalKey::Mail),
        winuser::VK_LAUNCH_MEDIA_SELECT => Some(LogicalKey::MediaSelect),
        /*winuser::VK_LAUNCH_APP1 => Some(LogicalKey::Launch_app1),
        winuser::VK_LAUNCH_APP2 => Some(LogicalKey::Launch_app2),*/
        winuser::VK_OEM_PLUS => Some(LogicalKey::Equals),
        winuser::VK_OEM_COMMA => Some(LogicalKey::Comma),
        winuser::VK_OEM_MINUS => Some(LogicalKey::Minus),
        winuser::VK_OEM_PERIOD => Some(LogicalKey::Period),
        winuser::VK_OEM_1 => map_text_keys(vkey),
        winuser::VK_OEM_2 => map_text_keys(vkey),
        winuser::VK_OEM_3 => map_text_keys(vkey),
        winuser::VK_OEM_4 => map_text_keys(vkey),
        winuser::VK_OEM_5 => map_text_keys(vkey),
        winuser::VK_OEM_6 => map_text_keys(vkey),
        winuser::VK_OEM_7 => map_text_keys(vkey),
        /* winuser::VK_OEM_8 => Some(LogicalKey::Oem_8), */
        winuser::VK_OEM_102 => Some(LogicalKey::OEM102),
        /*winuser::VK_PROCESSKEY => Some(LogicalKey::Processkey),
        winuser::VK_PACKET => Some(LogicalKey::Packet),
        winuser::VK_ATTN => Some(LogicalKey::Attn),
        winuser::VK_CRSEL => Some(LogicalKey::Crsel),
        winuser::VK_EXSEL => Some(LogicalKey::Exsel),
        winuser::VK_EREOF => Some(LogicalKey::Ereof),
        winuser::VK_PLAY => Some(LogicalKey::Play),
        winuser::VK_ZOOM => Some(LogicalKey::Zoom),
        winuser::VK_NONAME => Some(LogicalKey::Noname),
        winuser::VK_PA1 => Some(LogicalKey::Pa1),
        winuser::VK_OEM_CLEAR => Some(LogicalKey::Oem_clear),*/
        _ => None,
    }
}

pub fn handle_extended_keys(
    vkey: c_int,
    mut scancode: UINT,
    extended: bool,
) -> Option<(c_int, UINT)> {
    // Welcome to hell https://blog.molecular-matters.com/2011/09/05/properly-handling-keyboard-input/
    let vkey = match vkey {
        winuser::VK_SHIFT => unsafe {
            winuser::MapVirtualKeyA(scancode, winuser::MAPVK_VSC_TO_VK_EX) as _
        },
        winuser::VK_CONTROL => {
            if extended {
                winuser::VK_RCONTROL
            } else {
                winuser::VK_LCONTROL
            }
        }
        winuser::VK_MENU => {
            if extended {
                winuser::VK_RMENU
            } else {
                winuser::VK_LMENU
            }
        }
        _ => {
            match scancode {
                // This is only triggered when using raw input. Without this check, we get two events whenever VK_PAUSE is
                // pressed, the first one having scancode 0x1D but vkey VK_PAUSE...
                0x1D if vkey == winuser::VK_PAUSE => return None,
                // ...and the second having scancode 0x45 but an unmatched vkey!
                0x45 => winuser::VK_PAUSE,
                // VK_PAUSE and VK_SCROLL have the same scancode when using modifiers, alongside incorrect vkey values.
                0x46 => {
                    if extended {
                        scancode = 0x45;
                        winuser::VK_PAUSE
                    } else {
                        winuser::VK_SCROLL
                    }
                }
                _ => vkey,
            }
        }
    };
    Some((vkey, scancode))
}

pub fn process_key_params(
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<(u32, Option<LogicalKey>, bool)> {
    let scancode = ((lparam >> 16) & 0xff) as UINT;
    let extended = (lparam & 0x01000000) != 0;
    let is_repeat = (lparam & 0x7fff) != 0;
    handle_extended_keys(wparam as _, scancode, extended)
        .map(|(vkey, scancode)| (scancode, vkey_to_winit_vkey(vkey), is_repeat))
}

// This is needed as windows doesn't properly distinguish
// some virtual key codes for different keyboard layouts
fn map_text_keys(win_virtual_key: i32) -> Option<LogicalKey> {
    let char_key =
        unsafe { winuser::MapVirtualKeyA(win_virtual_key as u32, winuser::MAPVK_VK_TO_CHAR) }
            & 0x7FFF;
    match char::from_u32(char_key) {
        Some(';') => Some(LogicalKey::Semicolon),
        Some('/') => Some(LogicalKey::Slash),
        Some('`') => Some(LogicalKey::Grave),
        Some('[') => Some(LogicalKey::LBracket),
        Some(']') => Some(LogicalKey::RBracket),
        Some('\'') => Some(LogicalKey::Apostrophe),
        Some('\\') => Some(LogicalKey::Backslash),
        _ => None,
    }
}

bitflags!{
    pub struct ButtonsDown: u8 {
        const BUTTON_1 = 1 << 1;
        const BUTTON_2 = 1 << 2;
        const BUTTON_3 = 1 << 3;
        const BUTTON_4 = 1 << 4;
        const BUTTON_5 = 1 << 5;
        const BUTTON_6 = 1 << 6;
    }
}

impl PointerButton {
    pub(crate) fn into_flags(&self) -> ButtonsDown {
        use crate::event::PointerButtonInner::*;
        match self.0 {
            BUTTON_1 => ButtonsDown::BUTTON_1,
            BUTTON_2 => ButtonsDown::BUTTON_2,
            BUTTON_3 => ButtonsDown::BUTTON_3,
            BUTTON_4 => ButtonsDown::BUTTON_4,
            BUTTON_5 => ButtonsDown::BUTTON_5,
            BUTTON_6 => ButtonsDown::BUTTON_6,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PointerState {
    pub in_window: bool,
    pub force: Force,
    pub tilt: PointerTiltEvent,
    pub twist: f64,
    pub contact_area: PhysicalSize<f64>,
    pub position: PhysicalPosition<f64>,
    pub buttons_down: ButtonsDown,
}

#[derive(Default, Debug, Clone, Copy)]
struct FullPointerState {
    state: PointerState,
    last_button_down: Option<LastButtonDown>,
}

#[derive(Debug, Clone, Copy)]
struct LastButtonDown {
    button: PointerButton,
    time: DWORD,
    position: PhysicalPosition<f64>,
    multi_click_count: u32,
}

impl Default for PointerState {
    fn default() -> PointerState {
        PointerState {
            in_window: false,
            force: Force::Normalized(0.0),
            tilt: PointerTiltEvent::new_tilt_angle(0.0, 0.0),
            twist: 0.0,
            contact_area: PhysicalSize::new(1.0, 1.0),
            position: PhysicalPosition::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
            buttons_down: ButtonsDown::empty(),
        }
    }
}

#[derive(Clone)]
pub struct PointerTracker {
    pointer_info: HashMap<PointerId, FullPointerState>,
    call_legacy_capture_fns: bool,
}

impl PointerTracker {
    pub fn new() -> PointerTracker {
        PointerTracker {
            pointer_info: HashMap::new(),
            call_legacy_capture_fns: true,
        }
    }

    pub fn update_pointer_state(
        &mut self,
        pointer_id: PointerId,
        message_time: i32,
        window: HWND,
        new_state: impl FnOnce(PointerState) -> PointerState,
        mut send_event: impl FnMut(WindowEvent<'static>)
    ) {
        let entry = self.pointer_info.entry(pointer_id);
        if let Entry::Vacant(_) = entry {
            send_event(WindowEvent::PointerCreated(pointer_id));
        }
        let message_time = message_time as DWORD;

        let full_pointer_state = entry.or_default();
        let new_state = new_state(full_pointer_state.state);

        let &mut FullPointerState {
           state: PointerState {
                ref mut in_window,
                ref mut force,
                ref mut tilt,
                ref mut twist,
                ref mut contact_area,
                ref mut position,
                ref mut buttons_down,
            },
            ref mut last_button_down,
        } = full_pointer_state;

        if *in_window != new_state.in_window && new_state.in_window {
            *in_window = new_state.in_window;
            send_event(WindowEvent::PointerEntered(pointer_id));

            if self.call_legacy_capture_fns {
                unsafe {
                    // Calling TrackMouseEvent in order to receive mouse leave events.
                    winuser::TrackMouseEvent(&mut winuser::TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<winuser::TRACKMOUSEEVENT>() as DWORD,
                        dwFlags: winuser::TME_LEAVE,
                        hwndTrack: window,
                        dwHoverTime: winuser::HOVER_DEFAULT,
                    });
                }
            }
        }
        if *force != new_state.force {
            *force = new_state.force;
            send_event(WindowEvent::PointerForce(pointer_id, *force))
        }
        if *tilt != new_state.tilt {
            *tilt = new_state.tilt;
            send_event(WindowEvent::PointerTilt(pointer_id, *tilt))
        }
        if *twist != new_state.twist {
            *twist = new_state.twist;
            send_event(WindowEvent::PointerTwist(pointer_id, *twist))
        }
        if *contact_area != new_state.contact_area {
            *contact_area = new_state.contact_area;
            send_event(WindowEvent::PointerContactArea(pointer_id, *contact_area))
        }
        if *position != new_state.position {
            *position = new_state.position;
            send_event(WindowEvent::PointerMoved(pointer_id, *position))
        }
        if *buttons_down != new_state.buttons_down {
            let diff = *buttons_down ^ new_state.buttons_down;
            if diff != ButtonsDown::empty() {
                *buttons_down = new_state.buttons_down;

                if self.call_legacy_capture_fns && pointer_id == PointerId::MOUSE_ID {
                    unsafe {
                        match *buttons_down == ButtonsDown::empty() {
                            // Capture mouse input, allowing `window` to receive mouse events when
                            // the cursor is outside of the window.
                            false => {winuser::SetCapture(window);}
                            // Release mouse input, stopping windows on this thread from receiving
                            // mouse input when the cursor is outside the window.
                            true => {winuser::ReleaseCapture();}
                        }
                    }
                }

                let buttons = [
                    (PointerButton::BUTTON_1, ButtonsDown::BUTTON_1),
                    (PointerButton::BUTTON_2, ButtonsDown::BUTTON_2),
                    (PointerButton::BUTTON_3, ButtonsDown::BUTTON_3),
                    (PointerButton::BUTTON_4, ButtonsDown::BUTTON_4),
                    (PointerButton::BUTTON_5, ButtonsDown::BUTTON_5),
                    (PointerButton::BUTTON_6, ButtonsDown::BUTTON_6),
                ];

                let (double_click_time, cx_double_click, cy_double_click): (DWORD, f64, f64);
                match pointer_id {
                    PointerId::TouchId(_) |
                    PointerId::PenId(_) => unsafe {
                        let sub_key = "Software\\Microsoft\\Wisp\\Pen\\SysEventParameters\0";
                        let dist_value = "DblDist\0";
                        let time_value = "DblTime\0";

                        let mut time: DWORD = 0;
                        let result = winreg::RegGetValueA(
                            winreg::HKEY_CURRENT_USER,
                            sub_key.as_ptr() as *const _,
                            time_value.as_ptr() as *const _,
                            winreg::RRF_RT_REG_DWORD,
                            ptr::null_mut(),
                            &mut time as *mut DWORD as *mut _,
                            &mut (std::mem::size_of::<DWORD>() as DWORD),
                        );
                        double_click_time = match result {
                            0 => time,
                            _ => winuser::GetDoubleClickTime(),
                        };

                        let mut dist: DWORD = 0;
                        let result = winreg::RegGetValueA(
                            winreg::HKEY_CURRENT_USER,
                            sub_key.as_ptr() as *const _,
                            dist_value.as_ptr() as *const _,
                            winreg::RRF_RT_REG_DWORD,
                            ptr::null_mut(),
                            &mut dist as *mut DWORD as *mut _,
                            &mut (std::mem::size_of::<DWORD>() as DWORD),
                        );
                        cx_double_click = match result {
                            0 => dist as f64,
                            _ => winuser::GetSystemMetrics(winuser::SM_CXDOUBLECLK) as f64,
                        };
                        cy_double_click = cx_double_click;
                    },
                    PointerId::MouseId(_) => unsafe {
                        double_click_time = winuser::GetDoubleClickTime();
                        cx_double_click = winuser::GetSystemMetrics(winuser::SM_CXDOUBLECLK) as f64;
                        cy_double_click = winuser::GetSystemMetrics(winuser::SM_CYDOUBLECLK) as f64;
                    }
                }
                let scale_factor = unsafe{ winuser::GetDpiForWindow(window) as f64 / 96.0 };
                let cx_double_click = cx_double_click * scale_factor;
                let cy_double_click = cy_double_click * scale_factor;

                let mut last_click_set = false;
                for (pointer_button, button_flag) in buttons.iter().cloned() {
                    if diff.contains(button_flag) {
                        let multi_click_count = match last_button_down {
                            Some(lbd) if
                                lbd.button == pointer_button &&
                                (message_time.saturating_sub(lbd.time)) < double_click_time &&
                                (lbd.position.x - position.x).abs() < cx_double_click &&
                                (lbd.position.y - position.y).abs() < cy_double_click
                            => {
                                lbd.multi_click_count
                            },
                            _ => 0,
                        };
                        let is_down = buttons_down.contains(button_flag);

                        send_event(WindowEvent::PointerButton(
                            pointer_id,
                            PointerButtonEvent {
                                button: pointer_button,
                                is_down,
                                multi_click_count,
                            },
                        ));

                        if !last_click_set && !is_down {
                            last_click_set = true;
                            *last_button_down = Some(LastButtonDown {
                                button: pointer_button,
                                time: message_time,
                                position: *position,
                                multi_click_count: multi_click_count + 1,
                            });
                        }
                    }
                }
            }
        }
        if *in_window != new_state.in_window && !new_state.in_window {
            *in_window = new_state.in_window;
            send_event(WindowEvent::PointerLeft(pointer_id));
        }
    }

    pub fn destroy_pointer(&mut self, pointer_id: PointerId, mut send_event: impl FnMut(WindowEvent<'static>)) {
        if self.pointer_info.remove(&pointer_id).is_some() {
            send_event(WindowEvent::PointerDestroyed(pointer_id));
        }
    }
}
