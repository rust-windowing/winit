use crate::dpi::LogicalPosition;
use crate::event::{ModifiersState, MouseButton, MouseScrollDelta};

use std::convert::TryInto;
use web_sys::{MouseEvent, WheelEvent};

pub fn mouse_button(event: &MouseEvent) -> MouseButton {
    match event.button() {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        i => MouseButton::Other((i - 3).try_into().expect("very large mouse button value")),
    }
}

pub fn mouse_modifiers(event: &MouseEvent) -> ModifiersState {
    ModifiersState {
        shift: event.shift_key(),
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        logo: event.meta_key(),
    }
}

pub fn mouse_position(event: &MouseEvent) -> LogicalPosition {
    LogicalPosition {
        x: event.offset_x() as f64,
        y: event.offset_y() as f64,
    }
}

pub fn mouse_scroll_delta(event: &WheelEvent) -> Option<MouseScrollDelta> {
    let x = event.delta_x();
    let y = event.delta_y();

    match event.delta_mode() {
        WheelEvent::DOM_DELTA_LINE => Some(MouseScrollDelta::LineDelta(x as f32, y as f32)),
        WheelEvent::DOM_DELTA_PIXEL => Some(MouseScrollDelta::PixelDelta(LogicalPosition { x, y })),
        _ => None,
    }
}
