use crate::dpi::LogicalPosition;
use crate::event::{MouseButton, MouseScrollDelta};
use crate::keyboard::{Key, KeyCode, KeyLocation, ModifiersState};

use std::convert::TryInto;
use web_sys::{HtmlCanvasElement, KeyboardEvent, MouseEvent, WheelEvent};

pub fn mouse_button(event: &MouseEvent) -> MouseButton {
    match event.button() {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        i => MouseButton::Other((i - 3).try_into().expect("very large mouse button value")),
    }
}

pub fn mouse_modifiers(event: &MouseEvent) -> ModifiersState {
    let mut m = ModifiersState::empty();
    m.set(ModifiersState::SHIFT, event.shift_key());
    m.set(ModifiersState::CONTROL, event.ctrl_key());
    m.set(ModifiersState::ALT, event.alt_key());
    m.set(ModifiersState::SUPER, event.meta_key());
    m
}

pub fn mouse_position(event: &MouseEvent) -> LogicalPosition<f64> {
    LogicalPosition {
        x: event.offset_x() as f64,
        y: event.offset_y() as f64,
    }
}

pub fn mouse_delta(event: &MouseEvent) -> LogicalPosition<f64> {
    LogicalPosition {
        x: event.movement_x() as f64,
        y: event.movement_y() as f64,
    }
}

pub fn mouse_position_by_client(
    event: &MouseEvent,
    canvas: &HtmlCanvasElement,
) -> LogicalPosition<f64> {
    let bounding_client_rect = canvas.get_bounding_client_rect();
    LogicalPosition {
        x: event.client_x() as f64 - bounding_client_rect.x(),
        y: event.client_y() as f64 - bounding_client_rect.y(),
    }
}

pub fn mouse_scroll_delta(event: &WheelEvent) -> Option<MouseScrollDelta> {
    let x = event.delta_x();
    let y = -event.delta_y();

    match event.delta_mode() {
        WheelEvent::DOM_DELTA_LINE => Some(MouseScrollDelta::LineDelta(x as f32, y as f32)),
        WheelEvent::DOM_DELTA_PIXEL => {
            let delta = LogicalPosition::new(x, y).to_physical(super::scale_factor());
            Some(MouseScrollDelta::PixelDelta(delta))
        }
        _ => None,
    }
}

pub fn key_code(event: &KeyboardEvent) -> KeyCode {
    let code = event.code();
    KeyCode::from_key_code_attribute_value(&code)
}

pub fn key(event: &KeyboardEvent) -> Key<'static> {
    let key = event.key();
    let key = cached_string(key);
    Key::from_key_attribute_value(key)
}

pub fn key_text(event: &KeyboardEvent) -> Option<&'static str> {
    let key = event.key();
    let key = Key::from_key_attribute_value(&key);
    match &key {
        Key::Character(text) => Some(*text),
        Key::Tab => Some("\t"),
        Key::Enter => Some("\r"),
        Key::Space => Some(" "),
        _ => None,
    }
    .map(cached_string)
}

fn cached_string<S: AsRef<str>>(string: S) -> &'static str {
    use std::collections::HashSet;
    use std::sync::Mutex;

    lazy_static::lazy_static! {
        static ref STRING_CACHE: Mutex<HashSet<&'static str>> = Mutex::new(HashSet::new());
    };

    let string = string.as_ref();
    let mut cache = STRING_CACHE.lock().unwrap();
    if let Some(string) = cache.get(string) {
        *string
    } else {
        // borrowck couldn't quite figure out this one on its own
        let string: &'static str = Box::leak(String::from(string).into_boxed_str());
        cache.insert(string);
        string
    }
}

pub fn key_location(event: &KeyboardEvent) -> KeyLocation {
    let location = event.location();
    // As defined in the UIEvents specification
    // https://w3c.github.io/uievents/#idl-keyboardevent
    match location {
        0 => KeyLocation::Standard,
        1 => KeyLocation::Left,
        2 => KeyLocation::Right,
        3 => KeyLocation::Numpad,
        _ => KeyLocation::Standard,
    }
}

// TODO: What should be done about `KeyboardEvent.isComposing`?

pub fn keyboard_modifiers(key: &Key<'_>) -> ModifiersState {
    match key {
        Key::Shift => ModifiersState::SHIFT,
        Key::Control => ModifiersState::CONTROL,
        Key::Alt => ModifiersState::ALT,
        Key::Super => ModifiersState::SUPER,
        _ => ModifiersState::empty(),
    }
}

// pub fn codepoint(event: &KeyboardEvent) -> char {
//     // `event.key()` always returns a non-empty `String`. Therefore, this should
//     // never panic.
//     // https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/key
//     event.key().chars().next().unwrap()
// }
