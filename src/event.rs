//! The `Event` enum and assorted supporting types.
//!
//! These are sent to the closure given to [`EventLoop::run(...)`][event_loop_run], where they get
//! processed and used to modify the program state. For more details, see the root-level documentation.
//!
//! [event_loop_run]: ../event_loop/struct.EventLoop.html#method.run
use std::time::Instant;
use std::path::PathBuf;

use dpi::{LogicalPosition, LogicalSize};
use window::WindowId;

pub mod device;

/// A generic event.
#[derive(Clone, Debug, PartialEq)]
pub enum Event<T> {
    /// Emitted when the OS sends an event to a winit window.
    WindowEvent {
        window_id: WindowId,
        event: WindowEvent,
    },

    /// Emitted when a mouse device has generated input.
    MouseEvent(device::MouseId, device::MouseEvent),
    /// Emitted when a keyboard device has generated input.
    KeyboardEvent(device::KeyboardId, device::KeyboardEvent),
    /// Emitted when a gamepad/joystick device has generated input.
    GamepadEvent(device::GamepadHandle, device::GamepadEvent),

    /// Emitted when an event is sent from [`EventLoopProxy::send_event`](../event_loop/struct.EventLoopProxy.html#method.send_event)
    UserEvent(T),
    /// Emitted when new events arrive from the OS to be processed.
    NewEvents(StartCause),
    /// Emitted when all of the event loop's events have been processed and control flow is about
    /// to be taken away from the program.
    EventsCleared,

    /// Emitted when the event loop is being shut down. This is irreversable - if this event is
    /// emitted, it is guaranteed to be the last event emitted.
    LoopDestroyed,

    /// Emitted when the application has been suspended or resumed.
    ///
    /// The parameter is true if app was suspended, and false if it has been resumed.
    Suspended(bool),
}

impl<T> Event<T> {
    pub fn map_nonuser_event<U>(self) -> Result<Event<U>, Event<T>> {
        use self::Event::*;
        match self {
            UserEvent(_) => Err(self),
            WindowEvent{window_id, event} => Ok(WindowEvent{window_id, event}),
            MouseEvent(id, event) => Ok(MouseEvent(id, event)),
            KeyboardEvent(id, event) => Ok(KeyboardEvent(id, event)),
            GamepadEvent(id, event) => Ok(GamepadEvent(id, event)),
            NewEvents(cause) => Ok(NewEvents(cause)),
            EventsCleared => Ok(EventsCleared),
            LoopDestroyed => Ok(LoopDestroyed),
            Suspended(suspended) => Ok(Suspended(suspended)),
        }
    }
}

/// The reason the event loop is resuming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StartCause {
    /// Sent if the time specified by `ControlFlow::WaitUntil` has been reached. Contains the
    /// moment the timeout was requested and the requested resume time. The actual resume time is
    /// guaranteed to be equal to or after the requested resume time.
    ResumeTimeReached {
        start: Instant,
        requested_resume: Instant
    },

    /// Sent if the OS has new events to send to the window, after a wait was requested. Contains
    /// the moment the wait was requested and the resume time, if requested.
    WaitCancelled {
        start: Instant,
        requested_resume: Option<Instant>
    },

    /// Sent if the event loop is being resumed after the loop's control flow was set to
    /// `ControlFlow::Poll`.
    Poll,

    /// Sent once, immediately after `run` is called. Indicates that the loop was just initialized.
    Init
}

/// An event from a `Window`.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowEvent {
    /// The size of the window has changed. Contains the client area's new dimensions.
    Resized(LogicalSize),

    /// The position of the window has changed. Contains the window's new position.
    Moved(LogicalPosition),

    /// The window has been requested to close.
    CloseRequested,

    /// The window has been destroyed.
    Destroyed,

    /// A file has been dropped into the window.
    ///
    /// When the user drops multiple files at once, this event will be emitted for each file
    /// separately.
    DroppedFile(PathBuf),

    /// A file is being hovered over the window.
    ///
    /// When the user hovers multiple files at once, this event will be emitted for each file
    /// separately.
    HoveredFile(PathBuf),

    /// A file was hovered, but has exited the window.
    ///
    /// There will be a single `HoveredFileCancelled` event triggered even if multiple files were
    /// hovered.
    HoveredFileCancelled,

    /// The window received a unicode character.
    ReceivedCharacter(char),

    /// The window gained or lost focus.
    ///
    /// The parameter is true if the window has gained focus, and false if it has lost focus.
    Focused(bool),

    /// An event from the keyboard has been received.
    KeyboardInput(KeyboardInput),

    /// The cursor has moved on the window.
    CursorMoved {
        /// (x,y) coords in pixels relative to the top-left corner of the window. Because the range of this data is
        /// limited by the display area and it may have been transformed by the OS to implement effects such as cursor
        /// acceleration, it should not be used to implement non-cursor-like interactions such as 3D camera control.
        position: LogicalPosition,
        modifiers: ModifiersState
    },

    /// The cursor has entered the window.
    CursorEntered,

    /// The cursor has left the window.
    CursorLeft,

    /// A mouse wheel movement or touchpad scroll occurred.
    MouseWheel { delta: MouseScrollDelta, phase: TouchPhase, modifiers: ModifiersState },

    /// An mouse button press has been received.
    MouseInput { state: ElementState, button: MouseButton, modifiers: ModifiersState },


    /// Touchpad pressure event.
    ///
    /// At the moment, only supported on Apple forcetouch-capable macbooks.
    /// The parameters are: pressure level (value between 0 and 1 representing how hard the touchpad
    /// is being pressed) and stage (integer representing the click level).
    TouchpadPressure { pressure: f32, stage: i64 },

    /// The OS or application has requested that the window be redrawn.
    RedrawRequested,

    /// Touch event has been received
    Touch(Touch),

    /// The DPI factor of the window has changed.
    ///
    /// The following user actions can cause DPI changes:
    ///
    /// * Changing the display's resolution.
    /// * Changing the display's DPI factor (e.g. in Control Panel on Windows).
    /// * Moving the window to a display with a different DPI factor.
    ///
    /// For more information about DPI in general, see the [`dpi`](dpi/index.html) module.
    HiDpiFactorChanged(f64),
}

/// A keyboard input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct KeyboardInput {
    /// Identifies the physical key pressed
    ///
    /// This should not change if the user adjusts the host's keyboard map. Use when the physical location of the
    /// key is more important than the key's host GUI semantics, such as for movement controls in a first-person
    /// game.
    pub scancode: ScanCode,

    pub state: ElementState,

    /// Identifies the semantic meaning of the key
    ///
    /// Use when the semantics of the key are more important than the physical location of the key, such as when
    /// implementing appropriate behavior for "page up."
    pub virtual_keycode: Option<VirtualKeyCode>,

    /// Modifier keys active at the time of this input.
    ///
    /// This is tracked internally to avoid tracking errors arising from modifier key state changes when events from
    /// this device are not being delivered to the application, e.g. due to keyboard focus being elsewhere.
    pub modifiers: ModifiersState
}

/// Touch input state.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    /// The touch has been cancelled by the OS.
    ///
    /// This can occur in a variety of situations, such as the window losing focus.
    Cancelled,
}

/// A touch event.
///
/// Every event is guaranteed to start with a `Start` event, and may end with either an `End` or
/// `Cancelled` event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    pub phase: TouchPhase,
    pub location: LogicalPosition,
    /// Unique identifier of a finger.
    ///
    /// This may get reused by the system after the touch ends.
    pub id: u64
}

/// Hardware-dependent keyboard scan code.
pub type ScanCode = u32;

/// Identifier for a specific analog axis on some device.
pub type AxisId = u32;

/// Identifier for a specific button on some device.
pub type ButtonId = u32;

/// The input state of a key or button.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ElementState {
    Pressed,
    Released,
}

/// A button on a mouse.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8),
}

/// A difference in the mouse scroll wheel state.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
	PixelDelta(LogicalPosition),
}

/// Symbolic name of a keyboard key.
#[derive(Debug, Hash, Ord, PartialOrd, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum VirtualKeyCode {
    /// The '1' key over the letters.
    Key1,
    /// The '2' key over the letters.
    Key2,
    /// The '3' key over the letters.
    Key3,
    /// The '4' key over the letters.
    Key4,
    /// The '5' key over the letters.
    Key5,
    /// The '6' key over the letters.
    Key6,
    /// The '7' key over the letters.
    Key7,
    /// The '8' key over the letters.
    Key8,
    /// The '9' key over the letters.
    Key9,
    /// The '0' key over the 'O' and 'P' keys.
    Key0,

    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    /// The Escape key, next to F1.
    Escape,

    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,

    /// Print Screen/SysRq.
    Snapshot,
    /// Scroll Lock.
    Scroll,
    /// Pause/Break key, next to Scroll lock.
    Pause,

    /// `Insert`, next to Backspace.
    Insert,
    Home,
    Delete,
    End,
    PageDown,
    PageUp,

    Left,
    Up,
    Right,
    Down,

    /// The Backspace key, right over Enter.
    // TODO: rename
    Back,
    /// The Enter key.
    Return,
    /// The space bar.
    Space,

    /// The "Compose" key on Linux.
    Compose,

    Caret,

    Numlock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,

    AbntC1,
    AbntC2,
    Add,
    Apostrophe,
    Apps,
    At,
    Ax,
    Backslash,
    Calculator,
    Capital,
    Colon,
    Comma,
    Convert,
    Decimal,
    Divide,
    Equals,
    Grave,
    Kana,
    Kanji,
    LAlt,
    LBracket,
    LControl,
    LShift,
    LWin,
    Mail,
    MediaSelect,
    MediaStop,
    Minus,
    Multiply,
    Mute,
    MyComputer,
    NavigateForward, // also called "Prior"
    NavigateBackward, // also called "Next"
    NextTrack,
    NoConvert,
    NumpadComma,
    NumpadEnter,
    NumpadEquals,
    OEM102,
    Period,
    PlayPause,
    Power,
    PrevTrack,
    RAlt,
    RBracket,
    RControl,
    RShift,
    RWin,
    Semicolon,
    Slash,
    Sleep,
    Stop,
    Subtract,
    Sysrq,
    Tab,
    Underline,
    Unlabeled,
    VolumeDown,
    VolumeUp,
    Wake,
    WebBack,
    WebFavorites,
    WebForward,
    WebHome,
    WebRefresh,
    WebSearch,
    WebStop,
    Yen,
    Copy,
    Paste,
    Cut,
}

/// The current state of the keyboard modifiers
///
/// Each field of this struct represents a modifier and is `true` if this modifier is active.
#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
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
