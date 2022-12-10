use std::ffi::c_void;

use core_foundation::{
    base::CFRelease,
    data::{CFDataGetBytePtr, CFDataRef},
};
use objc2::rc::{Id, Shared};
use smol_str::SmolStr;

use super::appkit::{NSEvent, NSEventModifierFlags};
use super::window::WinitWindow;
use crate::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent},
    keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NativeKey, NativeKeyCode},
    platform::{modifier_supplement::KeyEventExtModifierSupplement, scancode::KeyCodeExtScancode},
    platform_impl::platform::{
        ffi,
        util::{get_kbd_type, Never},
    },
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyEventExtra {
    pub text_with_all_modifiers: Option<SmolStr>,
    pub key_without_modifiers: Key,
}

impl KeyEventExtModifierSupplement for KeyEvent {
    fn text_with_all_modifiers(&self) -> Option<&str> {
        self.platform_specific
            .text_with_all_modifiers
            .as_ref()
            .map(|s| s.as_str())
    }

    fn key_without_modifiers(&self) -> Key {
        self.platform_specific.key_without_modifiers.clone()
    }
}

pub fn get_modifierless_char(scancode: u16) -> Key {
    let mut string = [0; 16];
    let input_source;
    let layout;
    unsafe {
        input_source = ffi::TISCopyCurrentKeyboardLayoutInputSource();
        if input_source.is_null() {
            log::error!("`TISCopyCurrentKeyboardLayoutInputSource` returned null ptr");
            return Key::Unidentified(NativeKey::MacOS(scancode));
        }
        let layout_data =
            ffi::TISGetInputSourceProperty(input_source, ffi::kTISPropertyUnicodeKeyLayoutData);
        if layout_data.is_null() {
            CFRelease(input_source as *mut c_void);
            log::error!("`TISGetInputSourceProperty` returned null ptr");
            return Key::Unidentified(NativeKey::MacOS(scancode));
        }
        layout = CFDataGetBytePtr(layout_data as CFDataRef) as *const ffi::UCKeyboardLayout;
    }
    let keyboard_type = get_kbd_type();

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
        log::error!(
            "`UCKeyTranslate` returned with the non-zero value: {}",
            translate_result
        );
        return Key::Unidentified(NativeKey::MacOS(scancode));
    }
    if result_len == 0 {
        log::error!("`UCKeyTranslate` was succesful but gave a string of 0 length.");
        return Key::Unidentified(NativeKey::MacOS(scancode));
    }
    let chars = String::from_utf16_lossy(&string[0..result_len as usize]);
    Key::Character(SmolStr::new(chars))
}

fn get_logical_key_char(ns_event: &NSEvent, modifierless_chars: &str) -> Key {
    let string = ns_event
        .charactersIgnoringModifiers()
        .map(|s| s.to_string())
        .unwrap_or_else(String::new);
    if string.is_empty() {
        // Probably a dead key
        let first_char = modifierless_chars.chars().next();
        return Key::Dead(first_char);
    }
    Key::Character(SmolStr::new(string))
}

pub(crate) fn create_key_event(
    ns_event: &NSEvent,
    is_press: bool,
    is_repeat: bool,
    in_ime: bool,
    key_override: Option<KeyCode>,
) -> KeyEvent {
    use ElementState::{Pressed, Released};
    let state = if is_press { Pressed } else { Released };

    let scancode = ns_event.key_code();
    let mut physical_key = key_override
        .clone()
        .unwrap_or_else(|| KeyCode::from_scancode(scancode as u32));

    let text_with_all_modifiers: Option<SmolStr> = {
        if key_override.is_some() {
            None
        } else {
            let characters = ns_event
                .characters()
                .map(|s| s.to_string())
                .unwrap_or_else(String::new);
            if characters.is_empty() {
                None
            } else {
                if matches!(physical_key, KeyCode::Unidentified(_)) {
                    // The key may be one of the funky function keys
                    physical_key = extra_function_key_to_code(scancode, &characters);
                }
                Some(SmolStr::new(characters))
            }
        }
    };
    let key_from_code = code_to_key(physical_key.clone(), scancode);
    let logical_key;
    let key_without_modifiers;
    if !matches!(key_from_code, Key::Unidentified(_)) {
        logical_key = key_from_code.clone();
        key_without_modifiers = key_from_code;
    } else {
        //println!("Couldn't get key from code: {:?}", physical_key);
        key_without_modifiers = get_modifierless_char(scancode);

        let modifiers = NSEvent::modifierFlags(ns_event);
        let has_ctrl = modifiers.contains(NSEventModifierFlags::NSControlKeyMask);

        match text_with_all_modifiers.as_ref() {
            Some(text) if !has_ctrl => {
                // Only checking for ctrl here, not checking for alt because we DO want to
                // include its effect in the key. For example if -on the Germay layout- one
                // presses alt+8, the logical key should be "{"
                // Also not checking if this is a release event because then this issue would
                // still affect the key release.
                logical_key = Key::Character(text.clone());
            }
            _ => {
                let modifierless_chars = match key_without_modifiers.as_ref() {
                    Key::Character(ch) => ch,
                    _ => "",
                };
                logical_key = get_logical_key_char(ns_event, modifierless_chars);
            }
        }
    }
    let text = if in_ime || !is_press {
        None
    } else {
        logical_key.to_text().map(SmolStr::new)
    };
    let location = code_to_location(physical_key.clone());
    KeyEvent {
        location,
        logical_key,
        physical_key,
        repeat: is_repeat,
        state,
        text,
        platform_specific: KeyEventExtra {
            key_without_modifiers,
            text_with_all_modifiers,
        },
    }
}

pub fn code_to_key(code: KeyCode, scancode: u16) -> Key {
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

        // Other numpad keys all generate text on macOS (if I understand correctly)
        KeyCode::NumpadEnter => Key::Enter,

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
        _ => Key::Unidentified(NativeKey::MacOS(scancode)),
    }
}

pub fn code_to_location(code: KeyCode) -> KeyLocation {
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

// While F1-F20 have scancodes we can match on, we have to check against UTF-16
// constants for the rest.
// https://developer.apple.com/documentation/appkit/1535851-function-key_unicodes?preferredLanguage=occ
pub fn extra_function_key_to_code(scancode: u16, string: &str) -> KeyCode {
    if let Some(ch) = string.encode_utf16().next() {
        match ch {
            0xf718 => KeyCode::F21,
            0xf719 => KeyCode::F22,
            0xf71a => KeyCode::F23,
            0xf71b => KeyCode::F24,
            _ => KeyCode::Unidentified(NativeKeyCode::MacOS(scancode)),
        }
    } else {
        KeyCode::Unidentified(NativeKeyCode::MacOS(scancode))
    }
}

pub(super) fn event_mods(event: &NSEvent) -> ModifiersState {
    let flags = event.modifierFlags();
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
