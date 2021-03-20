use std::{collections::HashSet, ffi::c_void, os::raw::c_ushort, sync::Mutex};

use objc::msg_send;

use cocoa::{
    appkit::{NSEvent, NSEventModifierFlags},
    base::id,
};

use core_foundation::{base::CFRelease, data::CFDataGetBytePtr};

use crate::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NativeKeyCode},
    platform::{modifier_supplement::KeyEventExtModifierSupplement, scancode::KeyCodeExtScancode},
    platform_impl::platform::{
        ffi,
        util::{ns_string_to_rust, IdRef, Never},
        DEVICE_ID,
    },
};

lazy_static! {
    static ref KEY_STRINGS: Mutex<HashSet<&'static str>> = Mutex::new(HashSet::new());
}

fn insert_or_get_key_str(string: String) -> &'static str {
    let mut string_set = KEY_STRINGS.lock().unwrap();
    if let Some(contained) = string_set.get(string.as_str()) {
        return contained;
    }
    let static_str = Box::leak(string.into_boxed_str());
    string_set.insert(static_str);
    static_str
}

#[derive(Debug)]
pub enum EventWrapper {
    StaticEvent(Event<'static, Never>),
    EventProxy(EventProxy),
}

#[derive(Debug, PartialEq)]
pub enum EventProxy {
    DpiChangedProxy {
        ns_window: IdRef,
        suggested_size: LogicalSize<f64>,
        scale_factor: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyEventExtra {
    pub text_with_all_modifiers: Option<&'static str>,
    pub key_without_modifiers: Key<'static>,
}

impl KeyEventExtModifierSupplement for KeyEvent {
    fn text_with_all_modifiers(&self) -> Option<&str> {
        self.platform_specific.text_with_all_modifiers
    }

    fn key_without_modifiers(&self) -> Key<'static> {
        self.platform_specific.key_without_modifiers
    }
}

pub fn get_modifierless_char(scancode: u16) -> Key<'static> {
    let mut string = [0; 16];
    let input_source;
    let layout;
    unsafe {
        input_source = ffi::TISCopyCurrentKeyboardLayoutInputSource();
        if input_source.is_null() {
            log::error!("`TISCopyCurrentKeyboardLayoutInputSource` returned null ptr");
            return Key::Unidentified(NativeKeyCode::MacOS(scancode));
        }
        let layout_data =
            ffi::TISGetInputSourceProperty(input_source, ffi::kTISPropertyUnicodeKeyLayoutData);
        if layout_data.is_null() {
            CFRelease(input_source as *mut c_void);
            log::error!("`TISGetInputSourceProperty` returned null ptr");
            return Key::Unidentified(NativeKeyCode::MacOS(scancode));
        }
        layout = CFDataGetBytePtr(layout_data) as *const ffi::UCKeyboardLayout;
    }
    let keyboard_type = unsafe { ffi::LMGetKbdType() };

    let mut result_len = 0;
    let mut dead_keys = 0;
    let modifiers = 0;
    let translate_result = unsafe {
        ffi::UCKeyTranslate(
            layout,
            scancode,
            ffi::kUCKeyActionDisplay,
            modifiers,
            keyboard_type as u32,
            ffi::kUCKeyTranslateNoDeadKeysMask,
            &mut dead_keys,
            string.len() as ffi::UniCharCount,
            &mut result_len,
            string.as_mut_ptr(),
        )
    };
    unsafe {
        CFRelease(input_source as *mut c_void);
    }
    if translate_result != 0 {
        log::error!("`UCKeyTranslate` returned 0");
        return Key::Unidentified(NativeKeyCode::MacOS(scancode));
    }
    if result_len == 0 {
        log::error!("`UCKeyTranslate` was succesful but gave a string of 0 length.");
        return Key::Unidentified(NativeKeyCode::MacOS(scancode));
    }
    let chars = String::from_utf16_lossy(&string[0..result_len as usize]);
    Key::Character(insert_or_get_key_str(chars))
}

fn get_logical_key_char(ns_event: id, modifierless_chars: &str) -> Key<'static> {
    let characters: id = unsafe { msg_send![ns_event, charactersIgnoringModifiers] };
    let string = unsafe { ns_string_to_rust(characters) };
    if string.is_empty() {
        // Probably a dead key
        let first_char = modifierless_chars.chars().next();
        return Key::Dead(first_char);
    }
    Key::Character(insert_or_get_key_str(string))
}

pub fn create_key_event(
    ns_event: id,
    is_press: bool,
    is_repeat: bool,
    key_override: Option<KeyCode>,
) -> KeyEvent {
    let scancode = get_scancode(ns_event);
    let physical_key = key_override.unwrap_or_else(|| KeyCode::from_scancode(scancode as u32));

    let text_with_all_modifiers: Option<&'static str> = {
        if key_override.is_some() {
            None
        } else {
            let characters: id = unsafe { msg_send![ns_event, characters] };
            let characters = unsafe { ns_string_to_rust(characters) };
            if characters.is_empty() {
                None
            } else {
                Some(insert_or_get_key_str(characters))
            }
        }
    };
    let key_from_code = code_to_key(physical_key, scancode);
    let logical_key;
    let key_without_modifiers;
    if !matches!(key_from_code, Key::Unidentified(_)) {
        logical_key = key_from_code;
        key_without_modifiers = key_from_code;
    } else {
        //println!("Couldn't get key from code: {:?}", physical_key);
        key_without_modifiers = get_modifierless_char(scancode);

        let modifiers = unsafe { NSEvent::modifierFlags(ns_event) };
        let has_alt = modifiers.contains(NSEventModifierFlags::NSAlternateKeyMask);
        let has_ctrl = modifiers.contains(NSEventModifierFlags::NSControlKeyMask);
        if has_alt || has_ctrl || text_with_all_modifiers.is_none() || !is_press {
            let modifierless_chars = match key_without_modifiers {
                Key::Character(ch) => ch,
                _ => "",
            };
            logical_key = get_logical_key_char(ns_event, modifierless_chars);
        } else {
            logical_key = Key::Character(text_with_all_modifiers.unwrap());
        }
    }

    KeyEvent {
        location: code_to_location(physical_key),
        logical_key,
        physical_key,
        repeat: is_repeat,
        state: if is_press {
            ElementState::Pressed
        } else {
            ElementState::Released
        },
        text: None,
        platform_specific: KeyEventExtra {
            key_without_modifiers,
            text_with_all_modifiers,
        },
    }
}

fn code_to_key(code: KeyCode, scancode: u16) -> Key<'static> {
    match code {
        KeyCode::Enter => Key::Enter,
        KeyCode::Tab => Key::Tab,
        KeyCode::Space => Key::Space,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Escape => Key::Escape,
        KeyCode::SuperRight => Key::Super,
        KeyCode::SuperLeft => Key::Super,
        KeyCode::ShiftLeft => Key::Shift,
        KeyCode::AltLeft => Key::Alt,
        KeyCode::ControlLeft => Key::Control,
        KeyCode::ShiftRight => Key::Shift,
        KeyCode::AltRight => Key::Alt,
        KeyCode::ControlRight => Key::Control,

        KeyCode::NumLock => Key::NumLock,
        KeyCode::AudioVolumeUp => Key::AudioVolumeUp,
        KeyCode::AudioVolumeDown => Key::AudioVolumeDown,

        // TODO
        // KeyCode::NumpadDecimal => Some(0x41),
        // KeyCode::NumpadMultiply => Some(0x43),
        // KeyCode::NumpadAdd => Some(0x45),
        // KeyCode::NumpadDivide => Some(0x4b),
        // KeyCode::NumpadEnter => Some(0x4c),
        // KeyCode::NumpadSubtract => Some(0x4e),
        // KeyCode::NumpadEqual => Some(0x51),
        // KeyCode::Numpad0 => Some(0x52),
        // KeyCode::Numpad1 => Some(0x53),
        // KeyCode::Numpad2 => Some(0x54),
        // KeyCode::Numpad3 => Some(0x55),
        // KeyCode::Numpad4 => Some(0x56),
        // KeyCode::Numpad5 => Some(0x57),
        // KeyCode::Numpad6 => Some(0x58),
        // KeyCode::Numpad7 => Some(0x59),
        // KeyCode::Numpad8 => Some(0x5b),
        // KeyCode::Numpad9 => Some(0x5c),
        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F4 => Key::F4,
        KeyCode::F5 => Key::F5,
        KeyCode::F6 => Key::F6,
        KeyCode::F7 => Key::F7,
        KeyCode::F8 => Key::F8,
        KeyCode::F9 => Key::F9,
        KeyCode::F10 => Key::F10,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        KeyCode::F13 => Key::F13,
        KeyCode::F14 => Key::F14,
        KeyCode::F15 => Key::F15,
        KeyCode::F16 => Key::F16,
        KeyCode::F17 => Key::F17,
        KeyCode::F18 => Key::F18,
        KeyCode::F19 => Key::F19,
        KeyCode::F20 => Key::F20,

        KeyCode::Insert => Key::Insert,
        KeyCode::Home => Key::Home,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::Delete => Key::Delete,
        KeyCode::End => Key::End,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowUp => Key::ArrowUp,
        _ => Key::Unidentified(NativeKeyCode::MacOS(scancode)),
    }
}

fn code_to_location(code: KeyCode) -> KeyLocation {
    match code {
        KeyCode::SuperRight => KeyLocation::Right,
        KeyCode::SuperLeft => KeyLocation::Left,
        KeyCode::ShiftLeft => KeyLocation::Left,
        KeyCode::AltLeft => KeyLocation::Left,
        KeyCode::ControlLeft => KeyLocation::Left,
        KeyCode::ShiftRight => KeyLocation::Right,
        KeyCode::AltRight => KeyLocation::Right,
        KeyCode::ControlRight => KeyLocation::Right,

        KeyCode::NumLock => KeyLocation::Numpad,
        KeyCode::NumpadDecimal => KeyLocation::Numpad,
        KeyCode::NumpadMultiply => KeyLocation::Numpad,
        KeyCode::NumpadAdd => KeyLocation::Numpad,
        KeyCode::NumpadDivide => KeyLocation::Numpad,
        KeyCode::NumpadEnter => KeyLocation::Numpad,
        KeyCode::NumpadSubtract => KeyLocation::Numpad,
        KeyCode::NumpadEqual => KeyLocation::Numpad,
        KeyCode::Numpad0 => KeyLocation::Numpad,
        KeyCode::Numpad1 => KeyLocation::Numpad,
        KeyCode::Numpad2 => KeyLocation::Numpad,
        KeyCode::Numpad3 => KeyLocation::Numpad,
        KeyCode::Numpad4 => KeyLocation::Numpad,
        KeyCode::Numpad5 => KeyLocation::Numpad,
        KeyCode::Numpad6 => KeyLocation::Numpad,
        KeyCode::Numpad7 => KeyLocation::Numpad,
        KeyCode::Numpad8 => KeyLocation::Numpad,
        KeyCode::Numpad9 => KeyLocation::Numpad,

        _ => KeyLocation::Standard,
    }
}

// pub fn char_to_keycode(c: char) -> Option<VirtualKeyCode> {
//     // We only translate keys that are affected by keyboard layout.
//     //
//     // Note that since keys are translated in a somewhat "dumb" way (reading character)
//     // there is a concern that some combination, i.e. Cmd+char, causes the wrong
//     // letter to be received, and so we receive the wrong key.
//     //
//     // Implementation reference: https://github.com/WebKit/webkit/blob/82bae82cf0f329dbe21059ef0986c4e92fea4ba6/Source/WebCore/platform/cocoa/KeyEventCocoa.mm#L626
//     Some(match c {
//         'a' | 'A' => VirtualKeyCode::A,
//         'b' | 'B' => VirtualKeyCode::B,
//         'c' | 'C' => VirtualKeyCode::C,
//         'd' | 'D' => VirtualKeyCode::D,
//         'e' | 'E' => VirtualKeyCode::E,
//         'f' | 'F' => VirtualKeyCode::F,
//         'g' | 'G' => VirtualKeyCode::G,
//         'h' | 'H' => VirtualKeyCode::H,
//         'i' | 'I' => VirtualKeyCode::I,
//         'j' | 'J' => VirtualKeyCode::J,
//         'k' | 'K' => VirtualKeyCode::K,
//         'l' | 'L' => VirtualKeyCode::L,
//         'm' | 'M' => VirtualKeyCode::M,
//         'n' | 'N' => VirtualKeyCode::N,
//         'o' | 'O' => VirtualKeyCode::O,
//         'p' | 'P' => VirtualKeyCode::P,
//         'q' | 'Q' => VirtualKeyCode::Q,
//         'r' | 'R' => VirtualKeyCode::R,
//         's' | 'S' => VirtualKeyCode::S,
//         't' | 'T' => VirtualKeyCode::T,
//         'u' | 'U' => VirtualKeyCode::U,
//         'v' | 'V' => VirtualKeyCode::V,
//         'w' | 'W' => VirtualKeyCode::W,
//         'x' | 'X' => VirtualKeyCode::X,
//         'y' | 'Y' => VirtualKeyCode::Y,
//         'z' | 'Z' => VirtualKeyCode::Z,
//         '1' | '!' => VirtualKeyCode::Key1,
//         '2' | '@' => VirtualKeyCode::Key2,
//         '3' | '#' => VirtualKeyCode::Key3,
//         '4' | '$' => VirtualKeyCode::Key4,
//         '5' | '%' => VirtualKeyCode::Key5,
//         '6' | '^' => VirtualKeyCode::Key6,
//         '7' | '&' => VirtualKeyCode::Key7,
//         '8' | '*' => VirtualKeyCode::Key8,
//         '9' | '(' => VirtualKeyCode::Key9,
//         '0' | ')' => VirtualKeyCode::Key0,
//         '=' | '+' => VirtualKeyCode::Equals,
//         '-' | '_' => VirtualKeyCode::Minus,
//         ']' | '}' => VirtualKeyCode::RBracket,
//         '[' | '{' => VirtualKeyCode::LBracket,
//         '\'' | '"' => VirtualKeyCode::Apostrophe,
//         ';' | ':' => VirtualKeyCode::Semicolon,
//         '\\' | '|' => VirtualKeyCode::Backslash,
//         ',' | '<' => VirtualKeyCode::Comma,
//         '/' | '?' => VirtualKeyCode::Slash,
//         '.' | '>' => VirtualKeyCode::Period,
//         '`' | '~' => VirtualKeyCode::Grave,
//         _ => return None,
//     })
// }

// pub fn scancode_to_keycode(scancode: c_ushort) -> Option<VirtualKeyCode> {
//     Some(match scancode {
//         0x00 => VirtualKeyCode::A,
//         0x01 => VirtualKeyCode::S,
//         0x02 => VirtualKeyCode::D,
//         0x03 => VirtualKeyCode::F,
//         0x04 => VirtualKeyCode::H,
//         0x05 => VirtualKeyCode::G,
//         0x06 => VirtualKeyCode::Z,
//         0x07 => VirtualKeyCode::X,
//         0x08 => VirtualKeyCode::C,
//         0x09 => VirtualKeyCode::V,
//         //0x0a => World 1,
//         0x0b => VirtualKeyCode::B,
//         0x0c => VirtualKeyCode::Q,
//         0x0d => VirtualKeyCode::W,
//         0x0e => VirtualKeyCode::E,
//         0x0f => VirtualKeyCode::R,
//         0x10 => VirtualKeyCode::Y,
//         0x11 => VirtualKeyCode::T,
//         0x12 => VirtualKeyCode::Key1,
//         0x13 => VirtualKeyCode::Key2,
//         0x14 => VirtualKeyCode::Key3,
//         0x15 => VirtualKeyCode::Key4,
//         0x16 => VirtualKeyCode::Key6,
//         0x17 => VirtualKeyCode::Key5,
//         0x18 => VirtualKeyCode::Equals,
//         0x19 => VirtualKeyCode::Key9,
//         0x1a => VirtualKeyCode::Key7,
//         0x1b => VirtualKeyCode::Minus,
//         0x1c => VirtualKeyCode::Key8,
//         0x1d => VirtualKeyCode::Key0,
//         0x1e => VirtualKeyCode::RBracket,
//         0x1f => VirtualKeyCode::O,
//         0x20 => VirtualKeyCode::U,
//         0x21 => VirtualKeyCode::LBracket,
//         0x22 => VirtualKeyCode::I,
//         0x23 => VirtualKeyCode::P,
//         0x24 => VirtualKeyCode::Return,
//         0x25 => VirtualKeyCode::L,
//         0x26 => VirtualKeyCode::J,
//         0x27 => VirtualKeyCode::Apostrophe,
//         0x28 => VirtualKeyCode::K,
//         0x29 => VirtualKeyCode::Semicolon,
//         0x2a => VirtualKeyCode::Backslash,
//         0x2b => VirtualKeyCode::Comma,
//         0x2c => VirtualKeyCode::Slash,
//         0x2d => VirtualKeyCode::N,
//         0x2e => VirtualKeyCode::M,
//         0x2f => VirtualKeyCode::Period,
//         0x30 => VirtualKeyCode::Tab,
//         0x31 => VirtualKeyCode::Space,
//         0x32 => VirtualKeyCode::Grave,
//         0x33 => VirtualKeyCode::Back,
//         //0x34 => unkown,
//         0x35 => VirtualKeyCode::Escape,
//         0x36 => VirtualKeyCode::RWin,
//         0x37 => VirtualKeyCode::LWin,
//         0x38 => VirtualKeyCode::LShift,
//         //0x39 => Caps lock,
//         0x3a => VirtualKeyCode::LAlt,
//         0x3b => VirtualKeyCode::LControl,
//         0x3c => VirtualKeyCode::RShift,
//         0x3d => VirtualKeyCode::RAlt,
//         0x3e => VirtualKeyCode::RControl,
//         //0x3f => Fn key,
//         0x40 => VirtualKeyCode::F17,
//         0x41 => VirtualKeyCode::NumpadDecimal,
//         //0x42 -> unkown,
//         0x43 => VirtualKeyCode::NumpadMultiply,
//         //0x44 => unkown,
//         0x45 => VirtualKeyCode::NumpadAdd,
//         //0x46 => unkown,
//         0x47 => VirtualKeyCode::Numlock,
//         //0x48 => KeypadClear,
//         0x49 => VirtualKeyCode::VolumeUp,
//         0x4a => VirtualKeyCode::VolumeDown,
//         0x4b => VirtualKeyCode::NumpadDivide,
//         0x4c => VirtualKeyCode::NumpadEnter,
//         //0x4d => unkown,
//         0x4e => VirtualKeyCode::NumpadSubtract,
//         0x4f => VirtualKeyCode::F18,
//         0x50 => VirtualKeyCode::F19,
//         0x51 => VirtualKeyCode::NumpadEquals,
//         0x52 => VirtualKeyCode::Numpad0,
//         0x53 => VirtualKeyCode::Numpad1,
//         0x54 => VirtualKeyCode::Numpad2,
//         0x55 => VirtualKeyCode::Numpad3,
//         0x56 => VirtualKeyCode::Numpad4,
//         0x57 => VirtualKeyCode::Numpad5,
//         0x58 => VirtualKeyCode::Numpad6,
//         0x59 => VirtualKeyCode::Numpad7,
//         0x5a => VirtualKeyCode::F20,
//         0x5b => VirtualKeyCode::Numpad8,
//         0x5c => VirtualKeyCode::Numpad9,
//         0x5d => VirtualKeyCode::Yen,
//         //0x5e => JIS Ro,
//         //0x5f => unkown,
//         0x60 => VirtualKeyCode::F5,
//         0x61 => VirtualKeyCode::F6,
//         0x62 => VirtualKeyCode::F7,
//         0x63 => VirtualKeyCode::F3,
//         0x64 => VirtualKeyCode::F8,
//         0x65 => VirtualKeyCode::F9,
//         //0x66 => JIS Eisuu (macOS),
//         0x67 => VirtualKeyCode::F11,
//         //0x68 => JIS Kanna (macOS),
//         0x69 => VirtualKeyCode::F13,
//         0x6a => VirtualKeyCode::F16,
//         0x6b => VirtualKeyCode::F14,
//         //0x6c => unkown,
//         0x6d => VirtualKeyCode::F10,
//         //0x6e => unkown,
//         0x6f => VirtualKeyCode::F12,
//         //0x70 => unkown,
//         0x71 => VirtualKeyCode::F15,
//         0x72 => VirtualKeyCode::Insert,
//         0x73 => VirtualKeyCode::Home,
//         0x74 => VirtualKeyCode::PageUp,
//         0x75 => VirtualKeyCode::Delete,
//         0x76 => VirtualKeyCode::F4,
//         0x77 => VirtualKeyCode::End,
//         0x78 => VirtualKeyCode::F2,
//         0x79 => VirtualKeyCode::PageDown,
//         0x7a => VirtualKeyCode::F1,
//         0x7b => VirtualKeyCode::Left,
//         0x7c => VirtualKeyCode::Right,
//         0x7d => VirtualKeyCode::Down,
//         0x7e => VirtualKeyCode::Up,
//         //0x7f =>  unkown,
//         0xa => VirtualKeyCode::Caret,
//         _ => return None,
//     })
// }

// // While F1-F20 have scancodes we can match on, we have to check against UTF-16
// // constants for the rest.
// // https://developer.apple.com/documentation/appkit/1535851-function-key_unicodes?preferredLanguage=occ
// pub fn check_function_keys(string: &str) -> Option<VirtualKeyCode> {
//     if let Some(ch) = string.encode_utf16().next() {
//         return Some(match ch {
//             0xf718 => VirtualKeyCode::F21,
//             0xf719 => VirtualKeyCode::F22,
//             0xf71a => VirtualKeyCode::F23,
//             0xf71b => VirtualKeyCode::F24,
//             _ => return None,
//         });
//     }

//     None
// }

pub fn event_mods(event: id) -> ModifiersState {
    let flags = unsafe { NSEvent::modifierFlags(event) };
    let mut m = ModifiersState::empty();
    m.set(
        ModifiersState::SHIFT,
        flags.contains(NSEventModifierFlags::NSShiftKeyMask),
    );
    m.set(
        ModifiersState::CONTROL,
        flags.contains(NSEventModifierFlags::NSControlKeyMask),
    );
    m.set(
        ModifiersState::ALT,
        flags.contains(NSEventModifierFlags::NSAlternateKeyMask),
    );
    m.set(
        ModifiersState::SUPER,
        flags.contains(NSEventModifierFlags::NSCommandKeyMask),
    );
    m
}

pub fn get_scancode(event: cocoa::base::id) -> c_ushort {
    // In AppKit, `keyCode` refers to the position (scancode) of a key rather than its character,
    // and there is no easy way to navtively retrieve the layout-dependent character.
    // In winit, we use keycode to refer to the key's character, and so this function aligns
    // AppKit's terminology with ours.
    unsafe { msg_send![event, keyCode] }
}

pub unsafe fn modifier_event(
    ns_event: id,
    keymask: NSEventModifierFlags,
    was_key_pressed: bool,
) -> Option<WindowEvent<'static>> {
    let is_pressed = NSEvent::modifierFlags(ns_event).contains(keymask);
    if was_key_pressed != is_pressed {
        let scancode = get_scancode(ns_event);
        let mut key = KeyCode::from_scancode(scancode as u32);

        // When switching keyboard layout using Ctrl+Space, the Ctrl release event
        // has `KeyA` as its keycode which would produce an incorrect key event.
        // To avoid this, we detect this scenario and override the key with one
        // that should be reasonable
        if key == KeyCode::KeyA {
            key = match keymask {
                NSEventModifierFlags::NSAlternateKeyMask => KeyCode::AltLeft,
                NSEventModifierFlags::NSCommandKeyMask => KeyCode::SuperLeft,
                NSEventModifierFlags::NSControlKeyMask => KeyCode::ControlLeft,
                NSEventModifierFlags::NSShiftKeyMask => KeyCode::ShiftLeft,
                NSEventModifierFlags::NSFunctionKeyMask => KeyCode::Fn,
                NSEventModifierFlags::NSNumericPadKeyMask => KeyCode::NumLock,
                _ => {
                    error!("Unknown keymask hit. This indicates a developer error.");
                    KeyCode::Unidentified(NativeKeyCode::Unidentified)
                }
            };
        }

        Some(WindowEvent::KeyboardInput {
            device_id: DEVICE_ID,
            event: create_key_event(ns_event, is_pressed, false, Some(key)),
            is_synthetic: false,
        })

    // let scancode = get_scancode(ns_event);
    // let virtual_keycode = scancode_to_keycode(scancode);
    // #[allow(deprecated)]
    // Some(WindowEvent::KeyboardInput {
    //     device_id: DEVICE_ID,
    //     input: KeyboardInput {
    //         state,
    //         scancode: scancode as _,
    //         virtual_keycode,
    //         modifiers: event_mods(ns_event),
    //     },
    //     is_synthetic: false,
    // })
    } else {
        None
    }
}
