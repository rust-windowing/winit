use platform_impl;
use event::{AxisId, ButtonId, ElementState, KeyboardInput, MouseButton};

/// A hint suggesting the type of button that was pressed.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ButtonHint {
    Start,
    Select,

    /// The north face button.
    ///
    /// * Nintendo: X
    /// * Playstation: Triangle
    /// * XBox: Y
    North,
    /// The south face button.
    ///
    /// * Nintendo: B
    /// * Playstation: X
    /// * XBox: A
    South,
    /// The east face button.
    ///
    /// * Nintendo: A
    /// * Playstation: Circle
    /// * XBox: B
    East,
    /// The west face button.
    ///
    /// * Nintendo: Y
    /// * Playstation: Square
    /// * XBox: X
    West,

    LeftStick,
    RightStick,

    LeftTrigger,
    RightTrigger,

    LeftShoulder,
    RightShoulder,

    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

/// A hint suggesting the type of axis that moved.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AxisHint {
    LeftStickX,
    LeftStickY,

    RightStickX,
    RightStickY,

    LeftTrigger,
    RightTrigger,

    /// This is supposed to have a specialized meaning, referring to a point-of-view switch present on joysticks used
    /// for flight simulation. However, Xbox 360 controllers (and their derivatives) use a hat switch for the D-pad.
    HatSwitch,

    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeviceEvent {
    MouseEvent(MouseId, MouseEvent),
    KeyboardEvent(KeyboardId, KeyboardEvent),
    GamepadEvent(GamepadHandle, GamepadEvent),
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum MouseEvent {
    Added,
    Removed,
    Button {
        state: ElementState,
        button: MouseButton,
        button_id: ButtonId,
    },
    Moved(f64, f64),
    Wheel(f64, f64),
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum KeyboardEvent {
    Added,
    Removed,
    Input(KeyboardInput),
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum GamepadEvent {
    Added,
    Removed,
    Axis {
        axis: AxisId,
        hint: Option<AxisHint>,
        value: f64,
    },
    Button {
        button_id: ButtonId,
        hint: Option<ButtonHint>,
        state: ElementState,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MouseId(pub(crate) platform_impl::MouseId);
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyboardId(pub(crate) platform_impl::KeyboardId);
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GamepadHandle(pub(crate) platform_impl::GamepadHandle);

impl MouseId {
    /// Returns a dummy `MouseId`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `MouseId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub unsafe fn dummy() -> Self {
        MouseId(platform_impl::MouseId::dummy())
    }
}

impl KeyboardId {
    /// Returns a dummy `KeyboardId`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `KeyboardId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub unsafe fn dummy() -> Self {
        KeyboardId(platform_impl::KeyboardId::dummy())
    }
}

impl GamepadHandle {
    /// Returns a dummy `GamepadHandle`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `GamepadHandle`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub unsafe fn dummy() -> Self {
        GamepadHandle(platform_impl::GamepadHandle::dummy())
    }
}
