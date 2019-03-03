use platform_impl;
use event::{AxisId, ButtonId, ElementState, KeyboardInput, MouseButton};
use std::fmt;

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

    // @francesca64 review: why were these variants here? I don't see how it makes sense for the dpad
    // to have axes, since it's four separate buttons.
    // DPadUp,
    // DPadDown,
    // DPadLeft,
    // DPadRight,
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum MouseEvent {
    /// A mouse device has been added.
    Added,
    /// A mouse device has been removed.
    Removed,
    /// A mouse button has been pressed or released.
    Button {
        state: ElementState,
        button: MouseButton,
    },
    /// Change in physical position of a pointing device.
    ///
    /// This represents raw, unfiltered physical motion, NOT the position of the mouse. Accordingly,
    /// the values provided here are the change in position of the mouse since the previous `Moved`
    /// event.
    Moved(f64, f64),
    /// Change in rotation of mouse wheel.
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MouseId(pub(crate) platform_impl::MouseId);
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyboardId(pub(crate) platform_impl::KeyboardId);
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    pub fn rumble(&self, left_speed: f64, right_speed: f64) {
        self.0.rumble(left_speed, right_speed);
    }
}

impl fmt::Debug for MouseId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.0.fmt(f)
    }
}

impl fmt::Debug for KeyboardId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.0.fmt(f)
    }
}

impl fmt::Debug for GamepadHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.0.fmt(f)
    }
}
