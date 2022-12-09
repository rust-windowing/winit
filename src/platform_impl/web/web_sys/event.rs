use crate::dpi::LogicalPosition;
use crate::event::{MouseButton, MouseScrollDelta};
use crate::keyboard::{Key, KeyCode, KeyLocation, ModifiersState};

use smol_str::SmolStr;
use std::convert::TryInto;
use web_sys::{HtmlCanvasElement, KeyboardEvent, MouseEvent, PointerEvent, WheelEvent};

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
    let x = -event.delta_x();
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

pub fn key(event: &KeyboardEvent) -> Key {
    Key::from_key_attribute_value(&event.key())
}

pub fn key_text(event: &KeyboardEvent) -> Option<SmolStr> {
    let key = event.key();
    let key = Key::from_key_attribute_value(&key);
    match &key {
        Key::Character(text) => Some(text.clone()),
        Key::Tab => Some(SmolStr::new("\t")),
        Key::Enter => Some(SmolStr::new("\r")),
        Key::Space => Some(SmolStr::new(" ")),
        _ => None,
    }
    .map(SmolStr::new)
}

pub fn key_location(event: &KeyboardEvent) -> KeyLocation {
    match event.location() {
        KeyboardEvent::DOM_KEY_LOCATION_LEFT => KeyLocation::Left,
        KeyboardEvent::DOM_KEY_LOCATION_RIGHT => KeyLocation::Right,
        KeyboardEvent::DOM_KEY_LOCATION_NUMPAD => KeyLocation::Numpad,
        KeyboardEvent::DOM_KEY_LOCATION_STANDARD => KeyLocation::Standard,
        _ => KeyLocation::Standard,
    }
}

// TODO: What should be done about `KeyboardEvent.isComposing`?

pub fn keyboard_modifiers(key: &Key) -> ModifiersState {
    match key {
        Key::Shift => ModifiersState::SHIFT,
        Key::Control => ModifiersState::CONTROL,
        Key::Alt => ModifiersState::ALT,
        Key::Super => ModifiersState::SUPER,
        _ => ModifiersState::empty(),
    }
}

pub fn touch_position(event: &PointerEvent, _canvas: &HtmlCanvasElement) -> LogicalPosition<f64> {
    // TODO: Should this handle more, like `mouse_position_by_client` does?
    LogicalPosition {
        x: event.client_x() as f64,
        y: event.client_y() as f64,
    }
}
