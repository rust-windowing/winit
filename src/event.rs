//! The `Event` enum and assorted supporting types.
//!
//! These are sent to the closure given to [`EventLoop::run(...)`][event_loop_run], where they get
//! processed and used to modify the program state. For more details, see the root-level documentation.
//!
//! [event_loop_run]: ../event_loop/struct.EventLoop.html#method.run
use std::time::Instant;
use std::path::PathBuf;

use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::window::WindowId;
use crate::platform_impl;

/// Describes a generic event.
#[derive(Debug, PartialEq)]
pub enum Event<'a, T: 'static> {
    /// Emitted when the OS sends an event to a winit window.
    WindowEvent {
        window_id: WindowId,
        event: WindowEvent<'a>,
    },
    /// Emitted when the OS sends an event to a device.
    DeviceEvent {
        device_id: DeviceId,
        event: DeviceEvent,
    },
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

impl<'a, T> Event<'a, T> {
    pub fn map_nonuser_event<U>(self) -> Result<Event<'a, U>, Event<'a, T>> {
        use self::Event::*;
        match self {
            UserEvent(_) => Err(self),
            WindowEvent{window_id, event} => Ok(WindowEvent{window_id, event}),
            DeviceEvent{device_id, event} => Ok(DeviceEvent{device_id, event}),
            NewEvents(cause) => Ok(NewEvents(cause)),
            EventsCleared => Ok(EventsCleared),
            LoopDestroyed => Ok(LoopDestroyed),
            Suspended(suspended) => Ok(Suspended(suspended)),
        }
    }

    pub fn to_static(self) -> Option<Event<'static, T>> {
        use self::Event::*;
        match self {
            WindowEvent{window_id, event} => event.to_static().map(|event| WindowEvent{window_id, event}),
            UserEvent(e) => Some(UserEvent(e)),
            DeviceEvent{device_id, event} => Some(DeviceEvent{device_id, event}),
            NewEvents(cause) => Some(NewEvents(cause)),
            EventsCleared => Some(EventsCleared),
            LoopDestroyed => Some(LoopDestroyed),
            Suspended(suspended) => Some(Suspended(suspended)),
        }
    }
}

/// Describes the reason the event loop is resuming.
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

/// Describes an event from a `Window`.
#[derive(Debug, PartialEq)]
pub enum WindowEvent<'a> {
    /// The size of the window has changed. Contains the client area's new dimensions.
    Resized(PhysicalSize),

    /// The position of the window has changed. Contains the window's new position.
    Moved(PhysicalPosition),

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
    KeyboardInput { device_id: DeviceId, input: KeyboardInput },

    /// The cursor has moved on the window.
    CursorMoved {
        device_id: DeviceId,

        /// (x,y) coords in pixels relative to the top-left corner of the window. Because the range of this data is
        /// limited by the display area and it may have been transformed by the OS to implement effects such as cursor
        /// acceleration, it should not be used to implement non-cursor-like interactions such as 3D camera control.
        position: PhysicalPosition,
        modifiers: ModifiersState
    },

    /// The cursor has entered the window.
    CursorEntered { device_id: DeviceId },

    /// The cursor has left the window.
    CursorLeft { device_id: DeviceId },

    /// A mouse wheel movement or touchpad scroll occurred.
    MouseWheel { device_id: DeviceId, delta: MouseScrollDelta, phase: TouchPhase, modifiers: ModifiersState },

    /// An mouse button press has been received.
    MouseInput { device_id: DeviceId, state: ElementState, button: MouseButton, modifiers: ModifiersState },


    /// Touchpad pressure event.
    ///
    /// At the moment, only supported on Apple forcetouch-capable macbooks.
    /// The parameters are: pressure level (value between 0 and 1 representing how hard the touchpad
    /// is being pressed) and stage (integer representing the click level).
    TouchpadPressure { device_id: DeviceId, pressure: f32, stage: i64 },

    /// Motion on some analog axis. May report data redundant to other, more specific events.
    AxisMotion { device_id: DeviceId, axis: AxisId, value: f64 },

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
    /// After this event callback has been processed, the window will be resized to whatever value
    /// is pointed to by the `new_inner_size` reference. By default, this will contain the size suggested
    /// by the OS, but it can be changed to any value. If `new_inner_size` is set to `None`, no resizing
    /// will occur.
    ///
    /// For more information about DPI in general, see the [`dpi`](dpi/index.html) module.
    HiDpiFactorChanged {
        hidpi_factor: f64,
        new_inner_size: &'a mut Option<PhysicalSize>
    },
}

impl<'a> WindowEvent<'a> {
    pub fn to_static(self) -> Option<WindowEvent<'static>> {
        use self::WindowEvent::*;
        match self {
            Resized(size) => Some(Resized(size)),
            Moved(position) => Some(Moved(position)),
            CloseRequested => Some(CloseRequested),
            Destroyed => Some(Destroyed),
            DroppedFile(file) => Some(DroppedFile(file)),
            HoveredFile(file) => Some(HoveredFile(file)),
            HoveredFileCancelled => Some(HoveredFileCancelled),
            ReceivedCharacter(c) => Some(ReceivedCharacter(c)),
            Focused(focused) => Some(Focused(focused)),
            KeyboardInput { device_id, input } => Some(KeyboardInput { device_id, input }),
            CursorMoved { device_id, position, modifiers } => Some(CursorMoved { device_id, position, modifiers }),
            CursorEntered { device_id } => Some(CursorEntered { device_id }),
            CursorLeft { device_id } => Some(CursorLeft { device_id }),
            MouseWheel { device_id, delta, phase, modifiers } => Some(MouseWheel { device_id, delta, phase, modifiers }),
            MouseInput { device_id, state, button, modifiers } => Some(MouseInput { device_id, state, button, modifiers }),
            TouchpadPressure { device_id, pressure, stage } => Some(TouchpadPressure { device_id, pressure, stage }),
            AxisMotion { device_id, axis, value } => Some(AxisMotion { device_id, axis, value }),
            RedrawRequested => Some(RedrawRequested),
            Touch(touch) => Some(Touch(touch)),
            HiDpiFactorChanged{..} => None,
        }
    }
}

/// Identifier of an input device.
///
/// Whenever you receive an event arising from a particular input device, this event contains a `DeviceId` which
/// identifies its origin. Note that devices may be virtual (representing an on-screen cursor and keyboard focus) or
/// physical. Virtual devices typically aggregate inputs from multiple physical devices.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub(crate) platform_impl::DeviceId);

impl DeviceId {
    /// Returns a dummy `DeviceId`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `DeviceId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub unsafe fn dummy() -> Self {
        DeviceId(platform_impl::DeviceId::dummy())
    }
}

/// Represents raw hardware events that are not associated with any particular window.
///
/// Useful for interactions that diverge significantly from a conventional 2D GUI, such as 3D camera or first-person
/// game controls. Many physical actions, such as mouse movement, can produce both device and window events. Because
/// window events typically arise from virtual devices (corresponding to GUI cursors and keyboard focus) the device IDs
/// may not match.
///
/// Note that these events are delivered regardless of input focus.
#[derive(Clone, Debug, PartialEq)]
pub enum DeviceEvent {
    Added,
    Removed,

    /// Change in physical position of a pointing device.
    ///
    /// This represents raw, unfiltered physical motion. Not to be confused with `WindowEvent::CursorMoved`.
    MouseMotion {
        /// (x, y) change in position in unspecified units.
        ///
        /// Different devices may use different units.
        delta: (f64, f64),
    },

    /// Physical scroll event
    MouseWheel {
        delta: MouseScrollDelta,
    },

    /// Motion on some analog axis.  This event will be reported for all arbitrary input devices
    /// that winit supports on this platform, including mouse devices.  If the device is a mouse
    /// device then this will be reported alongside the MouseMotion event.
    Motion { axis: AxisId, value: f64 },

    Button { button: ButtonId, state: ElementState },
    Key(KeyboardInput),
    Text { codepoint: char },
}

/// Describes a keyboard input event.
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

/// Describes touch-screen input state.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled
}

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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    pub device_id: DeviceId,
    pub phase: TouchPhase,
    pub location: PhysicalPosition,
    /// unique identifier of a finger.
    pub id: u64
}

/// Hardware-dependent keyboard scan code.
pub type ScanCode = u32;

/// Identifier for a specific analog axis on some device.
pub type AxisId = u32;

/// Identifier for a specific button on some device.
pub type ButtonId = u32;

/// Describes the input state of a key.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ElementState {
    Pressed,
    Released,
}

/// Describes a button of a mouse controller.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8),
}

/// Describes a difference in the mouse scroll wheel state.
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

/// Symbolic name for a keyboard key.
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

/// Represents the current state of the keyboard modifiers
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
