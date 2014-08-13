#[deriving(Clone,Show)]
pub enum Event {
    /// The size of the window has changed.
    Resized(uint, uint),

    /// The position of the window has changed.
    Moved(int, int),

    /// The window has been closed.
    Closed,

    /// The window received a unicode character.
    ReceivedCharacter(char),

    /// The window gained or lost focus.
    /// 
    /// The parameter is true if the window has gained focus, and false if it has lost focus.
    Focused(bool),

    /// An event from the keyboard has been received.
    KeyboardInput(ElementState, ScanCode, Option<VirtualKeyCode>, KeyModifiers),

    /// The cursor has moved on the window.
    /// 
    /// The parameter are the (x,y) coords in pixels relative to the top-left corner of the window.
    MouseMoved((int, int)),

    /// A positive value indicates that the wheel was rotated forward, away from the user;
    ///  a negative value indicates that the wheel was rotated backward, toward the user.
    MouseWheel(i32),

    /// An event from the mouse has been received.
    MouseInput(ElementState, MouseButton),
}

pub type ScanCode = u8;

bitflags!(
    #[deriving(Show)]
    flags KeyModifiers: u8 {
        static LeftControlModifier = 1,
        static RightControlModifier = 2,
        static LeftShitModifier = 4,
        static RightShitModifier = 8,
        static LeftAltModifier = 16,
        static RightRightModifier = 32,
        static NumLockModifier = 64,
        static CapsLockModifier = 128
    }
)

#[deriving(Show, Hash, PartialEq, Eq, Clone)]
pub enum ElementState {
    Pressed,
    Released,
}

#[deriving(Show, Hash, PartialEq, Eq, Clone)]
pub enum MouseButton {
    LeftMouseButton,
    RightMouseButton,
    MiddleMouseButton,
    OtherMouseButton(u8),
}

#[deriving(Show, Hash, PartialEq, Eq, Clone)]
pub enum VirtualKeyCode {
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    A,
    AbntC1,
    AbntC2,
    Add,
    Apostrophe,
    Apps,
    At,
    Ax,
    B,
    Back,
    Backslash,
    C,
    Calculator,
    Capital,
    Colon,
    Comma,
    Convert,
    D,
    Decimal,
    Delete,
    Divide,
    Down,
    E,
    End,
    Equals,
    Escape,
    F,
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
    G,
    Grave,
    H,
    Home,
    I,
    Insert,
    J,
    K,
    Kana,
    Kanji,
    L,
    LCracket,
    LControl,
    Left,
    LMenu,
    LShift,
    LWin,
    M,
    Mail,
    MediaSelect,
    MediaStop,
    Minus,
    Multiply,
    Mute,
    MyComputer,
    N,
    NextTrack,
    NoConvert,
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
    NumpadComma,
    NumpadEnter,
    NumpadEquals,
    O,
    OEM102,
    P,
    PageDown,
    PageUp,
    Pause,
    Period,
    Playpause,
    Power,
    Prevtrack,
    Q,
    R,
    RBracket,
    RControl,
    Return,
    Right,
    RMenu,
    RShift,
    RWin,
    S,
    Scroll,
    Semicolon,
    Slash,
    Sleep,
    Snapshot,
    Space,
    Stop,
    Subtract,
    Sysrq,
    T,
    Tab,
    U,
    Underline,
    Unlabeled,
    Up,
    V,
    VolumeDown,
    VolumeUp,
    W,
    Wake,
    Webback,
    WebFavorites,
    WebForward,
    WebHome,
    WebRefresh,
    WebSearch,
    WebStop,
    X,
    Y,
    Yen,
    Z
}
