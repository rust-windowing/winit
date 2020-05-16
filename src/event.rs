//! The `Event` enum and assorted supporting types.
//!
//! These are sent to the closure given to [`EventLoop::run(...)`][event_loop_run], where they get
//! processed and used to modify the program state. For more details, see the root-level documentation.
//!
//! Some of these events represent different "parts" of a traditional event-handling loop. You could
//! approximate the basic ordering loop of [`EventLoop::run(...)`][event_loop_run] like this:
//!
//! ```rust,ignore
//! let mut control_flow = ControlFlow::Poll;
//! let mut start_cause = StartCause::Init;
//!
//! while control_flow != ControlFlow::Exit {
//!     event_handler(NewEvents(start_cause), ..., &mut control_flow);
//!
//!     for e in (window events, user events, device events) {
//!         event_handler(e, ..., &mut control_flow);
//!     }
//!     event_handler(MainEventsCleared, ..., &mut control_flow);
//!
//!     for w in (redraw windows) {
//!         event_handler(RedrawRequested(w), ..., &mut control_flow);
//!     }
//!     event_handler(RedrawEventsCleared, ..., &mut control_flow);
//!
//!     start_cause = wait_if_necessary(control_flow);
//! }
//!
//! event_handler(LoopDestroyed, ..., &mut control_flow);
//! ```
//!
//! This leaves out timing details like `ControlFlow::WaitUntil` but hopefully
//! describes what happens in what order.
//!
//! [event_loop_run]: crate::event_loop::EventLoop::run
use instant::Instant;
use std::{
    path::PathBuf,
    fmt,
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    platform_impl,
    window::{Theme, WindowId},
};

/// Describes a generic event.
///
/// See the module-level docs for more information on the event loop manages each event.
#[derive(Debug, PartialEq)]
pub enum Event<'a, T: 'static> {
    /// Emitted when new events arrive from the OS to be processed.
    ///
    /// This event type is useful as a place to put code that should be done before you start
    /// processing events, such as updating frame timing information for benchmarking or checking
    /// the [`StartCause`][crate::event::StartCause] to see if a timer set by
    /// [`ControlFlow::WaitUntil`](crate::event_loop::ControlFlow::WaitUntil) has elapsed.
    NewEvents(StartCause),

    AppEvent(AppEvent),

    /// Emitted when the OS sends an event to a winit window.
    WindowEvent(WindowId, WindowEvent),
    WindowEventImmediate(WindowId, WindowEventImmediate<'a>),

    RawPointerEvent(PointerDeviceId, RawPointerEvent),
    RawKeyboardEvent(KeyboardDeviceId, RawKeyboardEvent),

    /// Emitted when an event is sent from [`EventLoopProxy::send_event`](crate::event_loop::EventLoopProxy::send_event)
    UserEvent(T),

    /// Emitted when all of the event loop's input events have been processed and redraw processing
    /// is about to begin.
    ///
    /// This event is useful as a place to put your code that should be run after all
    /// state-changing events have been handled and you want to do stuff (updating state, performing
    /// calculations, etc) that happens as the "main body" of your event loop. If your program only draws
    /// graphics when something changes, it's usually better to do it in response to
    /// [`Event::RedrawRequested`](crate::event::Event::RedrawRequested), which gets emitted
    /// immediately after this event. Programs that draw graphics continuously, like most games,
    /// can render here unconditionally for simplicity.
    MainEventsCleared,

    /// Emitted after `MainEventsCleared` when a window should be redrawn.
    ///
    /// This gets triggered in two scenarios:
    /// - The OS has performed an operation that's invalidated the window's contents (such as
    ///   resizing the window).
    /// - The application has explicitly requested a redraw via
    ///   [`Window::request_redraw`](crate::window::Window::request_redraw).
    ///
    /// During each iteration of the event loop, Winit will aggregate duplicate redraw requests
    /// into a single event, to help avoid duplicating rendering work.
    ///
    /// Mainly of interest to applications with mostly-static graphics that avoid redrawing unless
    /// something changes, like most non-game GUIs.
    RedrawRequested(WindowId),

    /// Emitted after all `RedrawRequested` events have been processed and control flow is about to
    /// be taken away from the program. If there are no `RedrawRequested` events, it is emitted
    /// immediately after `MainEventsCleared`.
    ///
    /// This event is useful for doing any cleanup or bookkeeping work after all the rendering
    /// tasks have been completed.
    RedrawEventsCleared,

    /// Emitted when the event loop is being shut down.
    ///
    /// This is irreversable - if this event is emitted, it is guaranteed to be the last event that
    /// gets emitted. You generally want to treat this as an "do on quit" event.
    LoopDestroyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    /// Emitted when the application has been suspended.
    Suspended,

    /// Emitted when the application has been resumed.
    Resumed,
}

impl<'a, T> Event<'a, T> {
    pub fn map_nonuser_event<U>(self) -> Result<Event<'a, U>, Event<'a, T>> {
        use self::Event::*;
        match self {
            UserEvent(_) => Err(self),
            WindowEvent(window_id, event) => Ok(WindowEvent(window_id, event)),
            WindowEventImmediate(window_id, event) => Ok(WindowEventImmediate(window_id, event)),
            RawPointerEvent(pointer_id, event) => Ok(RawPointerEvent(pointer_id, event)),
            RawKeyboardEvent(keyboard_id, event) => Ok(RawKeyboardEvent(keyboard_id, event)),
            NewEvents(cause) => Ok(NewEvents(cause)),
            MainEventsCleared => Ok(MainEventsCleared),
            RedrawRequested(wid) => Ok(RedrawRequested(wid)),
            RedrawEventsCleared => Ok(RedrawEventsCleared),
            LoopDestroyed => Ok(LoopDestroyed),
            Suspended => Ok(Suspended),
            Resumed => Ok(Resumed),
        }
    }

    /// If the event doesn't contain a reference, turn it into an event with a `'static` lifetime.
    /// Otherwise, return `None`.
    pub fn to_static(self) -> Result<Event<'static, T>, Event<'a, T>> {
        use self::Event::*;
        match self {
            NewEvents(cause) => Ok(NewEvents(cause)),
            WindowEvent(window_id, event) => Ok(WindowEvent(window_id, event)),
            WindowEventImmediate(window_id, event) => Err(WindowEventImmediate(window_id, event)),
            RawPointerEvent(pointer_id, event) => Ok(RawPointerEvent(pointer_id, event)),
            RawKeyboardEvent(keyboard_id, event) => Ok(RawKeyboardEvent(keyboard_id, event)),
            Suspended => Ok(Suspended),
            Resumed => Ok(Resumed),
            UserEvent(e) => Ok(UserEvent(e)),
            MainEventsCleared => Ok(MainEventsCleared),
            RedrawRequested(wid) => Ok(RedrawRequested(wid)),
            RedrawEventsCleared => Ok(RedrawEventsCleared),
            LoopDestroyed => Ok(LoopDestroyed),
        }
    }
}

/// Describes the reason the event loop is resuming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartCause {
    /// Sent if the time specified by `ControlFlow::WaitUntil` has been reached. Contains the
    /// moment the timeout was requested and the requested resume time. The actual resume time is
    /// guaranteed to be equal to or after the requested resume time.
    ResumeTimeReached {
        start: Instant,
        requested_resume: Instant,
    },

    /// Sent if the OS has new events to send to the window, after a wait was requested. Contains
    /// the moment the wait was requested and the resume time, if requested.
    WaitCancelled {
        start: Instant,
        requested_resume: Option<Instant>,
    },

    /// Sent if the event loop is being resumed after the loop's control flow was set to
    /// `ControlFlow::Poll`.
    Poll,

    /// Sent once, immediately after `run` is called. Indicates that the loop was just initialized.
    Init,
}

/// Describes an event from a `Window`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum WindowEvent {
    /// The size of the window has changed. Contains the client area's new dimensions.
    Resized(PhysicalSize<u32>),

    /// The position of the window has changed. Contains the window's new position.
    Moved(PhysicalPosition<i32>),

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

    /// The window gained focus.
    FocusedGained,

    /// The window lost focus.
    FocustLost,

    /// The window received a unicode character.
    Char(char),

    KeyPress(KeyPress),

    /// The keyboard modifiers have changed.
    ///
    /// This is tracked internally to avoid tracking errors arising from modifier key state changes when events from
    /// this device are not being delivered to the application, e.g. due to keyboard focus being elsewhere.
    ///
    /// Platform-specific behavior:
    /// - **Web**: This API is currently unimplemented on the web. This isn't by design - it's an
    ///   issue, and it should get fixed - but it's the current state of the API.
    ModifiersChanged(ModifiersState),

    PointerCreated(PointerId),
    PointerDestroyed(PointerId),

    PointerMoved(PointerId, PhysicalPosition<f64>),

    PointerEntered(PointerId),
    PointerLeft(PointerId),

    PointerForce(PointerId, Force),

    PointerPress(PointerId, PointerPress),

    ScrollStarted,
    ScrollDiscrete(Vector<i32>),
    ScrollSmooth(Vector<f64>),
    ScrollEnded,

    /// The system window theme has changed.
    ///
    /// Applications might wish to react to this to change the theme of the content of the window
    /// when the system changes the window theme.
    ///
    /// At the moment this is only supported on Windows.
    ThemeChanged(Theme),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Vector<T> {
    pub x: T,
    pub y: T,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyPress {
    pub(crate) logical_key: Option<LogicalKey>,
    pub(crate) scan_code: u32,
    pub(crate) is_down: bool,
    pub(crate) is_repeat: bool,
    pub(crate) is_synthetic: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RawKeyPress {
    pub(crate) logical_key: Option<LogicalKey>,
    pub(crate) scan_code: u32,
    pub(crate) is_down: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PointerPress {
    pub(crate) button: PointerButton,
    pub(crate) is_down: bool,
    pub(crate) click_count: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RawPointerPress {
    pub(crate) button: PointerButton,
    pub(crate) is_down: bool,
}

impl KeyPress {
    pub fn logical_key(&self) -> Option<LogicalKey> {
        self.logical_key
    }
    pub fn scan_code(&self) -> u32 {
        self.scan_code
    }
    pub fn is_down(&self) -> bool {
        self.is_down
    }
    /// Is `true` if the user has held down the key long enough to send duplicate events.
    pub fn is_repeat(&self) -> bool {
        self.repeat_count
    }
    /// If set, the event was generated synthetically by winit
    /// in one of the following circumstances:
    ///
    /// * Synthetic key press events are generated for all keys pressed
    ///   when a window gains focus. Likewise, synthetic key release events
    ///   are generated for all keys pressed when a window goes out of focus.
    ///   ***Currently, this is only functional on X11 and Windows***
    ///
    /// Otherwise, this value is always `false`.
    pub fn is_synthetic(&self) -> bool {
        self.is_synthetic
    }
}

impl RawKeyPress {
    pub fn logical_key(&self) -> Option<LogicalKey> {
        self.logical_key
    }
    pub fn scan_code(&self) -> u32 {
        self.scan_code
    }
    pub fn is_down(&self) -> bool {
        self.is_down
    }
}

impl PointerPress {
    pub fn button(&self) -> PointerButton {
        self.button
    }
    pub fn is_down(&self) -> bool {
        self.is_down
    }
    /// The number of clicks the user has made in the same spot within the system's double-click
    /// interval. `1` is emitted on the first click, `2` is emitted on the second click, etc.
    pub fn click_count(&self) -> u32 {
        self.click_count
    }
}

impl RawPointerPress {
    pub fn button(&self) -> PointerButton {
        self.button
    }
    pub fn is_down(&self) -> bool {
        self.is_down
    }
}

#[derive(Debug, PartialEq)]
pub enum WindowEventImmediate<'a> {
    /// The window's scale factor has changed.
    ///
    /// The following user actions can cause DPI changes:
    ///
    /// * Changing the display's resolution.
    /// * Changing the display's scale factor (e.g. in Control Panel on Windows).
    /// * Moving the window to a display with a different scale factor.
    ///
    /// After this event callback has been processed, the window will be resized to whatever value
    /// is pointed to by the `PhysicalSize` reference. By default, this will contain the size suggested
    /// by the OS, but it can be changed to any value.
    ///
    /// For more information about DPI in general, see the [`dpi`](crate::dpi) module.
    ScaleFactorChanged(f64, &'a mut PhysicalSize<u32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PointerId {
    MouseId(MouseId),
    TouchId(TouchId),
    // PenId(PenId),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct PointerButton(PointerButtonInner);

/// We use an internal enum rather than exposing the variants directly so that the `BUTTON_n`
/// constants are formatted and exposed in a similar style to the `{MOUSE|TOUCH|PEN}_n` constants.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename = "PointerButton"))]
enum PointerButtonInner {
    Button0,
    Button1,
    Button2,
    Button3,
    Button4,
    Button5,
}

impl PointerButton {
    pub const MOUSE_LEFT: Self = Self::BUTTON_0;
    pub const MOUSE_RIGHT: Self = Self::BUTTON_1;
    pub const MOUSE_MIDDLE: Self = Self::BUTTON_2;
    pub const MOUSE_X1: Self = Self::BUTTON_3;
    pub const MOUSE_X2: Self = Self::BUTTON_4;

    pub const TOUCH_DOWN: Self = Self::BUTTON_0;

    // pub const PEN_DOWN: Self = Self::BUTTON_0;
    // pub const PEN_BARREL: Self = Self::BUTTON_1;
    // pub const PEN_ERASER: Self = Self::BUTTON_5;

    pub const BUTTON_0: Self = Self(PointerButtonInner::Button0);
    pub const BUTTON_1: Self = Self(PointerButtonInner::Button1);
    pub const BUTTON_2: Self = Self(PointerButtonInner::Button2);
    pub const BUTTON_3: Self = Self(PointerButtonInner::Button3);
    pub const BUTTON_4: Self = Self(PointerButtonInner::Button4);
    // pub const BUTTON_5: Self = Self(PointerButtonInner::Button5);

    pub fn as_u8(&self) -> u8 { self.0 }
    pub fn is_mouse_left(&self) -> bool { *self == Self::MOUSE_LEFT }
    pub fn is_mouse_right(&self) -> bool { *self == Self::MOUSE_RIGHT }
    pub fn is_mouse_middle(&self) -> bool { *self == Self::MOUSE_MIDDLE }
    pub fn is_mouse_x1(&self) -> bool { *self == Self::MOUSE_X1 }
    pub fn is_mouse_x2(&self) -> bool { *self == Self::MOUSE_X2 }
    pub fn is_touch_down(&self) -> bool { *self == Self::TOUCH_DOWN }
    // pub fn is_pen_down(&self) -> bool { *self == Self::PEN_DOWN }
    // pub fn is_pen_barrel(&self) -> bool { *self == Self::PEN_BARREL }
    // pub fn is_pen_eraser(&self) -> bool { *self == Self::PEN_ERASER }
    pub fn is_button_0(&self) -> bool { *self == Self::BUTTON_0 }
    pub fn is_button_1(&self) -> bool { *self == Self::BUTTON_1 }
    pub fn is_button_2(&self) -> bool { *self == Self::BUTTON_2 }
    pub fn is_button_3(&self) -> bool { *self == Self::BUTTON_3 }
    pub fn is_button_4(&self) -> bool { *self == Self::BUTTON_4 }
    // pub fn is_button_5(&self) -> bool { *self == Self::BUTTON_5 }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RawPointerEvent {
    /// A device has been added.
    Added,
    /// A device has been removed.
    Removed,
    Press(RawPointerPress),
    /// Relative change in physical position of a pointing device.
    ///
    /// This represents raw, unfiltered physical motion, NOT the position of the mouse. Accordingly,
    /// the values provided here are the change in position of the mouse since the previous
    /// `MovedRelative` event.
    MovedRelative(Vector<f64>),
    /// Change in absolute position of a pointing device.
    ///
    /// The `PhysicalPosition` value is the new position of the cursor relative to the desktop. This
    /// generally doesn't get output by standard mouse devices, but can get output from tablet devices.
    MovedAbsolute(PhysicalPosition<f64>),
    /// Change in rotation of mouse wheel.
    Wheel(Vector<f64>),
}

/// Raw keyboard events.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum RawKeyboardEvent {
    /// A keyboard device has been added.
    Added,
    /// A keyboard device has been removed.
    Removed,
    Press(RawKeyPress),
}

/// A typed identifier for a mouse device.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MouseId(pub(crate) platform_impl::MouseId);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TouchId(pub(crate) platform_impl::TouchId);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointerDeviceId(pub(crate) platform_impl::PointerDeviceId);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyboardDeviceId(pub(crate) platform_impl::KeyboardDeviceId);

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

impl PointerDeviceId {
    /// Returns a dummy `PointerDeviceId`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `PointerDeviceId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub unsafe fn dummy() -> Self {
        PointerDeviceId(platform_impl::PointerDeviceId::dummy())
    }
}

impl KeyboardDeviceId {
    /// Returns a dummy `KeyboardDeviceId`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `KeyboardDeviceId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub unsafe fn dummy() -> Self {
        KeyboardDeviceId(platform_impl::KeyboardDeviceId::dummy())
    }
}

impl fmt::Debug for MouseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.0.fmt(f)
    }
}

impl fmt::Debug for PointerDeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.0.fmt(f)
    }
}

impl fmt::Debug for KeyboardDeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.0.fmt(f)
    }
}

impl fmt::Debug for TouchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.0.fmt(f)
    }
}

/// Describes the force of a touch event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Force {
    /// On iOS, the force is calibrated so that the same number corresponds to
    /// roughly the same amount of pressure on the screen regardless of the
    /// device.
    Calibrated {
        /// The force of the touch, where a value of 1.0 represents the force of
        /// an average touch (predetermined by the system, not user-specific).
        ///
        /// The force reported by Apple Pencil is measured along the axis of the
        /// pencil. If you want a force perpendicular to the device, you need to
        /// calculate this value using the `altitude_angle` value.
        force: f64,
        /// The maximum possible force for a touch.
        ///
        /// The value of this field is sufficiently high to provide a wide
        /// dynamic range for values of the `force` field.
        max_possible_force: f64,
        /// The altitude (in radians) of the stylus.
        ///
        /// A value of 0 radians indicates that the stylus is parallel to the
        /// surface. The value of this property is Pi/2 when the stylus is
        /// perpendicular to the surface.
        altitude_angle: Option<f64>,
    },
    /// If the platform reports the force as normalized, we have no way of
    /// knowing how much pressure 1.0 corresponds to – we know it's the maximum
    /// amount of force, but as to how much force, you might either have to
    /// press really really hard, or not hard at all, depending on the device.
    Normalized(f64),
}

impl Force {
    /// Returns the force normalized to the range between 0.0 and 1.0 inclusive.
    /// Instead of normalizing the force, you should prefer to handle
    /// `Force::Calibrated` so that the amount of force the user has to apply is
    /// consistent across devices.
    pub fn normalized(&self) -> f64 {
        match self {
            Force::Calibrated {
                force,
                max_possible_force,
                altitude_angle,
            } => {
                let force = match altitude_angle {
                    Some(altitude_angle) => force / altitude_angle.sin(),
                    None => *force,
                };
                force / max_possible_force
            }
            Force::Normalized(force) => *force,
        }
    }
}

/// Symbolic name for a keyboard key.
#[derive(Debug, Hash, Ord, PartialOrd, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LogicalKey {
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
    // also called "Next"
    NavigateForward,
    // also called "Prior"
    NavigateBackward,
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

impl ModifiersState {
    /// Returns `true` if the shift key is pressed.
    pub fn shift(&self) -> bool {
        self.intersects(Self::SHIFT)
    }
    /// Returns `true` if the control key is pressed.
    pub fn ctrl(&self) -> bool {
        self.intersects(Self::CTRL)
    }
    /// Returns `true` if the alt key is pressed.
    pub fn alt(&self) -> bool {
        self.intersects(Self::ALT)
    }
    /// Returns `true` if the logo key is pressed.
    pub fn logo(&self) -> bool {
        self.intersects(Self::LOGO)
    }
}

bitflags! {
    /// Represents the current state of the keyboard modifiers
    ///
    /// Each flag represents a modifier and is set if this modifier is active.
    #[derive(Default)]
    pub struct ModifiersState: u32 {
        // left and right modifiers are currently commented out, but we should be able to support
        // them in a future release
        /// The "shift" key.
        const SHIFT = 0b100 << 0;
        // const LSHIFT = 0b010 << 0;
        // const RSHIFT = 0b001 << 0;
        /// The "control" key.
        const CTRL = 0b100 << 3;
        // const LCTRL = 0b010 << 3;
        // const RCTRL = 0b001 << 3;
        /// The "alt" key.
        const ALT = 0b100 << 6;
        // const LALT = 0b010 << 6;
        // const RALT = 0b001 << 6;
        /// This is the "windows" key on PC and "command" key on Mac.
        const LOGO = 0b100 << 9;
        // const LLOGO = 0b010 << 9;
        // const RLOGO = 0b001 << 9;
    }
}

#[cfg(feature = "serde")]
mod modifiers_serde {
    use super::ModifiersState;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Default, Serialize, Deserialize)]
    #[serde(default)]
    #[serde(rename = "ModifiersState")]
    pub struct ModifiersStateSerialize {
        pub shift: bool,
        pub ctrl: bool,
        pub alt: bool,
        pub logo: bool,
    }

    impl Serialize for ModifiersState {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let s = ModifiersStateSerialize {
                shift: self.shift(),
                ctrl: self.ctrl(),
                alt: self.alt(),
                logo: self.logo(),
            };
            s.serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for ModifiersState {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let ModifiersStateSerialize {
                shift,
                ctrl,
                alt,
                logo,
            } = ModifiersStateSerialize::deserialize(deserializer)?;
            let mut m = ModifiersState::empty();
            m.set(ModifiersState::SHIFT, shift);
            m.set(ModifiersState::CTRL, ctrl);
            m.set(ModifiersState::ALT, alt);
            m.set(ModifiersState::LOGO, logo);
            Ok(m)
        }
    }
}
