use std::path::PathBuf;
use {WindowId, DeviceId};

use keyboard_types::{KeyEvent, CompositionEvent};

#[derive(Clone, Debug)]
pub enum Event {
    WindowEvent {
        window_id: WindowId,
        event: WindowEvent,
    },
    DeviceEvent {
        device_id: DeviceId,
        event: DeviceEvent,
    },
    Awakened,
}

#[derive(Clone, Debug)]
pub enum WindowEvent {

    /// The size of the window has changed.
    Resized(u32, u32),

    /// The position of the window has changed.
    Moved(i32, i32),

    /// The window has been closed.
    Closed,

    /// A file has been dropped into the window.
    DroppedFile(PathBuf),

    /// The window received a unicode character.
    ReceivedCharacter(char),

    /// The window gained or lost focus.
    ///
    /// The parameter is true if the window has gained focus, and false if it has lost focus.
    Focused(bool),

    /// An event from the keyboard has been received.
    KeyboardInput { device_id: DeviceId, input: KeyEvent },

    CompositionInput { device_id: DeviceId, input: CompositionEvent },

    /// The cursor has moved on the window.
    ///
    /// `position` is (x,y) coords in pixels relative to the top-left corner of the window. Because the range of this
    /// data is limited by the display area and it may have been transformed by the OS to implement effects such as
    /// mouse acceleration, it should not be used to implement non-cursor-like interactions such as 3D camera control.
    MouseMoved { device_id: DeviceId, position: (f64, f64) },

    /// The cursor has entered the window.
    MouseEntered { device_id: DeviceId },

    /// The cursor has left the window.
    MouseLeft { device_id: DeviceId },

    /// A mouse wheel movement or touchpad scroll occurred.
    MouseWheel { device_id: DeviceId, delta: MouseScrollDelta, phase: TouchPhase },

    /// An mouse button press has been received.
    MouseInput { device_id: DeviceId, state: ElementState, button: MouseButton },

    /// Touchpad pressure event.
    ///
    /// At the moment, only supported on Apple forcetouch-capable macbooks.
    /// The parameters are: pressure level (value between 0 and 1 representing how hard the touchpad
    /// is being pressed) and stage (integer representing the click level).
    TouchpadPressure { device_id: DeviceId, pressure: f32, stage: i64 },

    /// Motion on some analog axis not otherwise handled. May overlap with mouse motion.
    AxisMotion { device_id: DeviceId, axis: AxisId, value: f64 },

    /// The window needs to be redrawn.
    Refresh,

    /// App has been suspended or resumed.
    ///
    /// The parameter is true if app was suspended, and false if it has been resumed.
    Suspended(bool),

    /// Touch event has been received
    Touch(Touch)
}

/// Represents raw hardware events that are not associated with any particular window.
///
/// Useful for interactions that diverge significantly from a conventional 2D GUI, such as 3D camera or first-person
/// game controls. Many physical actions, such as mouse movement, can produce both device and window events. Because
/// window events typically arise from virtual devices (corresponding to GUI cursors and keyboard focus) the device IDs
/// may not match.
///
/// Note that these events are delivered regardless of input focus.
#[derive(Clone, Debug)]
pub enum DeviceEvent {
    Added,
    Removed,
    Motion { axis: AxisId, value: f64 },
    Button { button: ButtonId, state: ElementState },
    Text { codepoint: char },
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled
}

#[derive(Debug, Clone, Copy)]
/// Represents touch event
///
/// Every time user touches screen new Start event with some finger id is generated.
/// When the finger is removed from the screen End event with same id is generated.
///
/// For every id there will be at least 2 events with phases Start and End (or Cancelled).
/// There may be 0 or more Move events.
///
///
/// Depending on platform implementation id may or may not be reused by system after End event.
///
/// Gesture regonizer using this event should assume that Start event received with same id
/// as previously received End event is a new finger and has nothing to do with an old one.
///
/// Touch may be cancelled if for example window lost focus.
pub struct Touch {
    pub device_id: DeviceId,
    pub phase: TouchPhase,
    pub location: (f64,f64),
    /// unique identifier of a finger.
    pub id: u64
}

pub type ScanCode = u32;

/// Identifier for a specific analog axis on some device.
pub type AxisId = u32;

/// Identifier for a specific button on some device.
pub type ButtonId = u32;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum ElementState {
    Pressed,
    Released,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseScrollDelta {
	/// Amount in lines or rows to scroll in the horizontal
	/// and vertical directions.
	///
	/// Positive values indicate movement forward
	/// (away from the user) or rightwards.
	LineDelta(f32, f32),
	/// Amount in pixels to scroll in the horizontal and
	/// vertical direction.
	///
	/// Scroll events are expressed as a PixelDelta if
	/// supported by the device (eg. a touchpad) and
	/// platform.
	PixelDelta(f32, f32)
}

/// Represents the current state of the keyboard modifiers
///
/// Each field of this struct represents a modifier and is `true` if this modifier is active.
#[derive(Default, Debug, Clone, Copy)]
pub struct ModifiersState {
    /// The "shift" key
    pub shift: bool,
    /// The "control" key
    pub ctrl: bool,
    /// The "alt" key
    pub alt: bool,
    /// The "logo" key
    ///
    /// This is the "windows" key on PC and "command" key on Mac.
    pub logo: bool
}
