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
    // TODO: Fill out stub.
    let code = event.code();
    KeyCode::Enter
}

pub fn key(event: &KeyboardEvent) -> Key<'static> {
    // TODO: Fill out stub.
    Key::Enter
}

pub fn key_text(event: &KeyboardEvent) -> Option<&'static str> {
    // TODO: Fill out stub.
    None
}

pub fn key_location(event: &KeyboardEvent) -> KeyLocation {
    // TODO: Fill out stub.
    KeyLocation::Standard
}

pub fn key_repeat(event: &KeyboardEvent) -> bool {
    // TODO: Fill out stub.
    false
}

pub fn keyboard_modifiers(event: &KeyboardEvent) -> ModifiersState {
    let mut m = ModifiersState::empty();
    m.set(ModifiersState::SHIFT, event.shift_key());
    m.set(ModifiersState::CONTROL, event.ctrl_key());
    m.set(ModifiersState::ALT, event.alt_key());
    m.set(ModifiersState::SUPER, event.meta_key());
    m
}

pub fn codepoint(event: &KeyboardEvent) -> char {
    // `event.key()` always returns a non-empty `String`. Therefore, this should
    // never panic.
    // https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/key
    event.key().chars().next().unwrap()
}
