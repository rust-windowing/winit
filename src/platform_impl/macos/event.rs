use std::os::raw::c_ushort;

use objc2::rc::{Id, Shared};

use super::appkit::{NSEvent, NSEventModifierFlags};
use super::window::WinitWindow;
use crate::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyboardInput, ModifiersState, VirtualKeyCode, WindowEvent},
    platform_impl::platform::{util::Never, DEVICE_ID},
};

#[derive(Debug)]
pub(crate) enum EventWrapper {
    StaticEvent(Event<'static, Never>),
    EventProxy(EventProxy),
}

#[derive(Debug)]
pub(crate) enum EventProxy {
    DpiChangedProxy {
        window: Id<WinitWindow, Shared>,
        suggested_size: LogicalSize<f64>,
        scale_factor: f64,
    },
}

pub fn char_to_keycode(c: char) -> Option<VirtualKeyCode> {
    // We only translate keys that are affected by keyboard layout.
    //
    // Note that since keys are translated in a somewhat "dumb" way (reading character)
    // there is a concern that some combination, i.e. Cmd+char, causes the wrong
    // letter to be received, and so we receive the wrong key.
    //
    // Implementation reference: https://github.com/WebKit/webkit/blob/82bae82cf0f329dbe21059ef0986c4e92fea4ba6/Source/WebCore/platform/cocoa/KeyEventCocoa.mm#L626
    Some(match c {
        'a' | 'A' => VirtualKeyCode::A,
        'b' | 'B' => VirtualKeyCode::B,
        'c' | 'C' => VirtualKeyCode::C,
        'd' | 'D' => VirtualKeyCode::D,
        'e' | 'E' => VirtualKeyCode::E,
        'f' | 'F' => VirtualKeyCode::F,
        'g' | 'G' => VirtualKeyCode::G,
        'h' | 'H' => VirtualKeyCode::H,
        'i' | 'I' => VirtualKeyCode::I,
        'j' | 'J' => VirtualKeyCode::J,
        'k' | 'K' => VirtualKeyCode::K,
        'l' | 'L' => VirtualKeyCode::L,
        'm' | 'M' => VirtualKeyCode::M,
        'n' | 'N' => VirtualKeyCode::N,
        'o' | 'O' => VirtualKeyCode::O,
        'p' | 'P' => VirtualKeyCode::P,
        'q' | 'Q' => VirtualKeyCode::Q,
        'r' | 'R' => VirtualKeyCode::R,
        's' | 'S' => VirtualKeyCode::S,
        't' | 'T' => VirtualKeyCode::T,
        'u' | 'U' => VirtualKeyCode::U,
        'v' | 'V' => VirtualKeyCode::V,
        'w' | 'W' => VirtualKeyCode::W,
        'x' | 'X' => VirtualKeyCode::X,
        'y' | 'Y' => VirtualKeyCode::Y,
        'z' | 'Z' => VirtualKeyCode::Z,
        '1' | '!' => VirtualKeyCode::Key1,
        '2' | '@' => VirtualKeyCode::Key2,
        '3' | '#' => VirtualKeyCode::Key3,
        '4' | '$' => VirtualKeyCode::Key4,
        '5' | '%' => VirtualKeyCode::Key5,
        '6' | '^' => VirtualKeyCode::Key6,
        '7' | '&' => VirtualKeyCode::Key7,
        '8' | '*' => VirtualKeyCode::Key8,
        '9' | '(' => VirtualKeyCode::Key9,
        '0' | ')' => VirtualKeyCode::Key0,
        '=' | '+' => VirtualKeyCode::Equals,
        '-' | '_' => VirtualKeyCode::Minus,
        ']' | '}' => VirtualKeyCode::RBracket,
        '[' | '{' => VirtualKeyCode::LBracket,
        '\'' | '"' => VirtualKeyCode::Apostrophe,
        ';' | ':' => VirtualKeyCode::Semicolon,
        '\\' | '|' => VirtualKeyCode::Backslash,
        ',' | '<' => VirtualKeyCode::Comma,
        '/' | '?' => VirtualKeyCode::Slash,
        '.' | '>' => VirtualKeyCode::Period,
        '`' | '~' => VirtualKeyCode::Grave,
        _ => return None,
    })
}

pub fn scancode_to_keycode(scancode: c_ushort) -> Option<VirtualKeyCode> {
    Some(match scancode {
        0x00 => VirtualKeyCode::A,
        0x01 => VirtualKeyCode::S,
        0x02 => VirtualKeyCode::D,
        0x03 => VirtualKeyCode::F,
        0x04 => VirtualKeyCode::H,
        0x05 => VirtualKeyCode::G,
        0x06 => VirtualKeyCode::Z,
        0x07 => VirtualKeyCode::X,
        0x08 => VirtualKeyCode::C,
        0x09 => VirtualKeyCode::V,
        //0x0a => World 1,
        0x0b => VirtualKeyCode::B,
        0x0c => VirtualKeyCode::Q,
        0x0d => VirtualKeyCode::W,
        0x0e => VirtualKeyCode::E,
        0x0f => VirtualKeyCode::R,
        0x10 => VirtualKeyCode::Y,
        0x11 => VirtualKeyCode::T,
        0x12 => VirtualKeyCode::Key1,
        0x13 => VirtualKeyCode::Key2,
        0x14 => VirtualKeyCode::Key3,
        0x15 => VirtualKeyCode::Key4,
        0x16 => VirtualKeyCode::Key6,
        0x17 => VirtualKeyCode::Key5,
        0x18 => VirtualKeyCode::Equals,
        0x19 => VirtualKeyCode::Key9,
        0x1a => VirtualKeyCode::Key7,
        0x1b => VirtualKeyCode::Minus,
        0x1c => VirtualKeyCode::Key8,
        0x1d => VirtualKeyCode::Key0,
        0x1e => VirtualKeyCode::RBracket,
        0x1f => VirtualKeyCode::O,
        0x20 => VirtualKeyCode::U,
        0x21 => VirtualKeyCode::LBracket,
        0x22 => VirtualKeyCode::I,
        0x23 => VirtualKeyCode::P,
        0x24 => VirtualKeyCode::Return,
        0x25 => VirtualKeyCode::L,
        0x26 => VirtualKeyCode::J,
        0x27 => VirtualKeyCode::Apostrophe,
        0x28 => VirtualKeyCode::K,
        0x29 => VirtualKeyCode::Semicolon,
        0x2a => VirtualKeyCode::Backslash,
        0x2b => VirtualKeyCode::Comma,
        0x2c => VirtualKeyCode::Slash,
        0x2d => VirtualKeyCode::N,
        0x2e => VirtualKeyCode::M,
        0x2f => VirtualKeyCode::Period,
        0x30 => VirtualKeyCode::Tab,
        0x31 => VirtualKeyCode::Space,
        0x32 => VirtualKeyCode::Grave,
        0x33 => VirtualKeyCode::Back,
        //0x34 => unkown,
        0x35 => VirtualKeyCode::Escape,
        0x36 => VirtualKeyCode::RWin,
        0x37 => VirtualKeyCode::LWin,
        0x38 => VirtualKeyCode::LShift,
        //0x39 => Caps lock,
        0x3a => VirtualKeyCode::LAlt,
        0x3b => VirtualKeyCode::LControl,
        0x3c => VirtualKeyCode::RShift,
        0x3d => VirtualKeyCode::RAlt,
        0x3e => VirtualKeyCode::RControl,
        //0x3f => Fn key,
        0x40 => VirtualKeyCode::F17,
        0x41 => VirtualKeyCode::NumpadDecimal,
        //0x42 -> unkown,
        0x43 => VirtualKeyCode::NumpadMultiply,
        //0x44 => unkown,
        0x45 => VirtualKeyCode::NumpadAdd,
        //0x46 => unkown,
        0x47 => VirtualKeyCode::Numlock,
        //0x48 => KeypadClear,
        0x49 => VirtualKeyCode::VolumeUp,
        0x4a => VirtualKeyCode::VolumeDown,
        0x4b => VirtualKeyCode::NumpadDivide,
        0x4c => VirtualKeyCode::NumpadEnter,
        //0x4d => unkown,
        0x4e => VirtualKeyCode::NumpadSubtract,
        0x4f => VirtualKeyCode::F18,
        0x50 => VirtualKeyCode::F19,
        0x51 => VirtualKeyCode::NumpadEquals,
        0x52 => VirtualKeyCode::Numpad0,
        0x53 => VirtualKeyCode::Numpad1,
        0x54 => VirtualKeyCode::Numpad2,
        0x55 => VirtualKeyCode::Numpad3,
        0x56 => VirtualKeyCode::Numpad4,
        0x57 => VirtualKeyCode::Numpad5,
        0x58 => VirtualKeyCode::Numpad6,
        0x59 => VirtualKeyCode::Numpad7,
        0x5a => VirtualKeyCode::F20,
        0x5b => VirtualKeyCode::Numpad8,
        0x5c => VirtualKeyCode::Numpad9,
        0x5d => VirtualKeyCode::Yen,
        //0x5e => JIS Ro,
        //0x5f => unkown,
        0x60 => VirtualKeyCode::F5,
        0x61 => VirtualKeyCode::F6,
        0x62 => VirtualKeyCode::F7,
        0x63 => VirtualKeyCode::F3,
        0x64 => VirtualKeyCode::F8,
        0x65 => VirtualKeyCode::F9,
        //0x66 => JIS Eisuu (macOS),
        0x67 => VirtualKeyCode::F11,
        //0x68 => JIS Kanna (macOS),
        0x69 => VirtualKeyCode::F13,
        0x6a => VirtualKeyCode::F16,
        0x6b => VirtualKeyCode::F14,
        //0x6c => unkown,
        0x6d => VirtualKeyCode::F10,
        //0x6e => unkown,
        0x6f => VirtualKeyCode::F12,
        //0x70 => unkown,
        0x71 => VirtualKeyCode::F15,
        0x72 => VirtualKeyCode::Insert,
        0x73 => VirtualKeyCode::Home,
        0x74 => VirtualKeyCode::PageUp,
        0x75 => VirtualKeyCode::Delete,
        0x76 => VirtualKeyCode::F4,
        0x77 => VirtualKeyCode::End,
        0x78 => VirtualKeyCode::F2,
        0x79 => VirtualKeyCode::PageDown,
        0x7a => VirtualKeyCode::F1,
        0x7b => VirtualKeyCode::Left,
        0x7c => VirtualKeyCode::Right,
        0x7d => VirtualKeyCode::Down,
        0x7e => VirtualKeyCode::Up,
        //0x7f =>  unkown,
        0xa => VirtualKeyCode::Caret,
        _ => return None,
    })
}

// While F1-F20 have scancodes we can match on, we have to check against UTF-16
// constants for the rest.
// https://developer.apple.com/documentation/appkit/1535851-function-key_unicodes?preferredLanguage=occ
pub fn check_function_keys(string: &str) -> Option<VirtualKeyCode> {
    if let Some(ch) = string.encode_utf16().next() {
        return Some(match ch {
            0xf718 => VirtualKeyCode::F21,
            0xf719 => VirtualKeyCode::F22,
            0xf71a => VirtualKeyCode::F23,
            0xf71b => VirtualKeyCode::F24,
            _ => return None,
        });
    }

    None
}

pub(super) fn event_mods(event: &NSEvent) -> ModifiersState {
    let flags = event.modifierFlags();
    let mut m = ModifiersState::empty();
    m.set(
        ModifiersState::SHIFT,
        flags.contains(NSEventModifierFlags::NSShiftKeyMask),
    );
    m.set(
        ModifiersState::CTRL,
        flags.contains(NSEventModifierFlags::NSControlKeyMask),
    );
    m.set(
        ModifiersState::ALT,
        flags.contains(NSEventModifierFlags::NSAlternateKeyMask),
    );
    m.set(
        ModifiersState::LOGO,
        flags.contains(NSEventModifierFlags::NSCommandKeyMask),
    );
    m
}

pub(super) fn modifier_event(
    event: &NSEvent,
    keymask: NSEventModifierFlags,
    was_key_pressed: bool,
) -> Option<WindowEvent<'static>> {
    if !was_key_pressed && event.modifierFlags().contains(keymask)
        || was_key_pressed && !event.modifierFlags().contains(keymask)
    {
        let state = if was_key_pressed {
            ElementState::Released
        } else {
            ElementState::Pressed
        };

        let scancode = event.scancode();
        let virtual_keycode = scancode_to_keycode(scancode);
        #[allow(deprecated)]
        Some(WindowEvent::KeyboardInput {
            device_id: DEVICE_ID,
            input: KeyboardInput {
                state,
                scancode: scancode as _,
                virtual_keycode,
                modifiers: event_mods(event),
            },
            is_synthetic: false,
        })
    } else {
        None
    }
}
