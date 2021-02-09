use super::constants;
use crate::event::{device, ElementState};

pub fn gamepad_button(code: usize, pressed: bool) -> device::GamepadEvent {
    let button_id = code as u32;
    let button = constants::button_code(code);

    let state = if pressed {
        ElementState::Pressed
    } else {
        ElementState::Released
    };

    device::GamepadEvent::Button {
        button_id,
        button,
        state,
    }
}

pub fn gamepad_axis(code: usize, value: f64) -> device::GamepadEvent {
    let axis_id = code as u32;
    let axis = constants::axis_code(code);

    device::GamepadEvent::Axis {
        axis_id,
        axis,
        value,
        stick: true,
    }
}

pub fn gamepad_stick(
    x_code: usize,
    y_code: usize,
    x_value: f64,
    y_value: f64,
    side: device::Side,
) -> device::GamepadEvent {
    let x_id = x_code as u32;
    let y_id = y_code as u32;

    device::GamepadEvent::Stick {
        x_id,
        y_id,
        x_value,
        y_value,
        side,
    }
}
