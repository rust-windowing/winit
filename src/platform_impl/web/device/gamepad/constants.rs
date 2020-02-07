use crate::event::device::{GamepadAxis, GamepadButton};

pub(crate) static BUTTONS: [GamepadButton; 16] = [
    GamepadButton::South,
    GamepadButton::East,
    GamepadButton::West,
    GamepadButton::North,
    GamepadButton::LeftTrigger,
    GamepadButton::RightTrigger,
    GamepadButton::LeftShoulder,
    GamepadButton::RightShoulder,
    GamepadButton::Select,
    GamepadButton::Start,
    GamepadButton::LeftStick,
    GamepadButton::RightStick,
    GamepadButton::DPadUp,
    GamepadButton::DPadDown,
    GamepadButton::DPadLeft,
    GamepadButton::DPadRight,
];

pub(crate) static AXES: [GamepadAxis; 6] = [
    GamepadAxis::LeftStickX,
    GamepadAxis::LeftStickY,
    GamepadAxis::RightStickX,
    GamepadAxis::RightStickY,
    GamepadAxis::LeftTrigger,
    GamepadAxis::RightTrigger,
];

pub(crate) fn button_code(index: usize) -> Option<GamepadButton> {
    BUTTONS.get(index).map(|ev| ev.clone())
}

pub(crate) fn axis_code(index: usize) -> Option<GamepadAxis> {
    AXES.get(index).map(|ev| ev.clone())
}
