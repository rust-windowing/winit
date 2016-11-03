use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Event {
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
    KeyboardInput(ElementState, ScanCode, Option<VirtualKeyCode>),

    /// The cursor has moved on the window.
    ///
    /// The parameter are the (x,y) coords in pixels relative to the top-left corner of the window.
    MouseMoved(i32, i32),

    /// The cursor has entered the window.
    MouseEntered,

    /// The cursor has left the window.
    MouseLeft,

    /// A mouse wheel movement or touchpad scroll occurred.
    MouseWheel(MouseScrollDelta, TouchPhase),

    /// An event from the mouse has been received.
    MouseInput(ElementState, MouseButton),

    /// Touchpad pressure event.
    ///
    /// At the moment, only supported on Apple forcetouch-capable macbooks.
    /// The parameters are: pressure level (value between 0 and 1 representing how hard the touchpad
    /// is being pressed) and stage (integer representing the click level).
    TouchpadPressure(f32, i64),

    /// The event loop was woken up by another thread.
    Awakened,

    /// The window needs to be redrawn.
    Refresh,

    /// App has been suspended or resumed.
    ///
    /// The parameter is true if app was suspended, and false if it has been resumed.
    Suspended(bool),


    /// Touch event has been received
    Touch(Touch)
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
    pub phase: TouchPhase,
    pub location: (f64,f64),
    /// unique identifier of a finger.
    pub id: u64
}

pub type ScanCode = u8;

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

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
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
    LMenu,
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
    RMenu,
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
}
