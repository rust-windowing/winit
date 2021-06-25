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
use std::path::PathBuf;

use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    window::Theme,
    window::WindowId,
};

pub mod device;

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

    /// Emitted when the OS sends an event to a winit window.
    WindowEvent {
        window_id: WindowId,
        event: WindowEvent<'a>,
    },

    /// Emitted when a mouse device has generated input.
    MouseEvent(device::MouseId, device::MouseEvent),
    /// Emitted when a keyboard device has generated input.
    KeyboardEvent(device::KeyboardId, device::KeyboardEvent),
    HidEvent(device::HidId, device::HidEvent),
    /// Emitted when a gamepad/joystick device has generated input.
    GamepadEvent(device::GamepadHandle, device::GamepadEvent),

    /// Emitted when an event is sent from [`EventLoopProxy::send_event`](crate::event_loop::EventLoopProxy::send_event)
    UserEvent(T),

    /// Emitted when the application has been suspended.
    Suspended,

    /// Emitted when the application has been resumed.
    Resumed,

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
    /// This is irreversible - if this event is emitted, it is guaranteed to be the last event that
    /// gets emitted. You generally want to treat this as an "do on quit" event.
    LoopDestroyed,
}

impl<T: Clone> Clone for Event<'static, T> {
    fn clone(&self) -> Self {
        use self::Event::*;
        match self {
            WindowEvent { window_id, event } => WindowEvent {
                window_id: *window_id,
                event: event.clone(),
            },
            UserEvent(event) => UserEvent(event.clone()),
            MouseEvent(id, event) => MouseEvent(id.clone(), event.clone()),
            KeyboardEvent(id, event) => KeyboardEvent(id.clone(), event.clone()),
            HidEvent(id, event) => HidEvent(id.clone(), event.clone()),
            GamepadEvent(id, event) => GamepadEvent(id.clone(), event.clone()),
            NewEvents(cause) => NewEvents(cause.clone()),
            MainEventsCleared => MainEventsCleared,
            RedrawRequested(wid) => RedrawRequested(*wid),
            RedrawEventsCleared => RedrawEventsCleared,
            LoopDestroyed => LoopDestroyed,
            Suspended => Suspended,
            Resumed => Resumed,
        }
    }
}

impl<'a, T> Event<'a, T> {
    pub fn map_nonuser_event<U>(self) -> Result<Event<'a, U>, Event<'a, T>> {
        use self::Event::*;
        match self {
            UserEvent(_) => Err(self),
            WindowEvent { window_id, event } => Ok(WindowEvent { window_id, event }),
            MouseEvent(id, event) => Ok(MouseEvent(id, event)),
            KeyboardEvent(id, event) => Ok(KeyboardEvent(id, event)),
            HidEvent(id, event) => Ok(HidEvent(id, event)),
            GamepadEvent(id, event) => Ok(GamepadEvent(id, event)),
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
    pub fn to_static(self) -> Option<Event<'static, T>> {
        use self::Event::*;
        match self {
            WindowEvent { window_id, event } => event
                .to_static()
                .map(|event| WindowEvent { window_id, event }),
            UserEvent(event) => Some(UserEvent(event)),
            MouseEvent(id, event) => Some(MouseEvent(id, event)),
            KeyboardEvent(id, event) => Some(KeyboardEvent(id, event)),
            HidEvent(id, event) => Some(HidEvent(id, event)),
            GamepadEvent(id, event) => Some(GamepadEvent(id, event)),
            NewEvents(cause) => Some(NewEvents(cause)),
            MainEventsCleared => Some(MainEventsCleared),
            RedrawRequested(wid) => Some(RedrawRequested(wid)),
            RedrawEventsCleared => Some(RedrawEventsCleared),
            LoopDestroyed => Some(LoopDestroyed),
            Suspended => Some(Suspended),
            Resumed => Some(Resumed),
        }
    }
}

/// The reason the event loop is resuming.
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
#[derive(Debug, PartialEq)]
pub enum WindowEvent<'a> {
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

    /// The window received a unicode character.
    ReceivedCharacter(char),

    /// The window gained or lost focus.
    ///
    /// The parameter is true if the window has gained focus, and false if it has lost focus.
    Focused(bool),

    /// An event from the keyboard has been received.
    KeyboardInput(KeyboardInput),

    /// The keyboard modifiers have changed.
    ///
    /// Platform-specific behavior:
    /// - **Web**: This API is currently unimplemented on the web. This isn't by design - it's an
    ///   issue, and it should get fixed - but it's the current state of the API.
    ModifiersChanged(ModifiersState),

    /// The cursor has moved on the window.
    CursorMoved {
        /// (x,y) coords in pixels relative to the top-left corner of the window. Because the range of this data is
        /// limited by the display area and it may have been transformed by the OS to implement effects such as cursor
        /// acceleration, it should not be used to implement non-cursor-like interactions such as 3D camera control.
        position: PhysicalPosition<f64>,
        #[deprecated = "Deprecated in favor of WindowEvent::ModifiersChanged"]
        modifiers: ModifiersState,
    },

    /// The cursor has entered the window.
    CursorEntered,

    /// The cursor has left the window.
    CursorLeft,

    /// A mouse wheel movement or touchpad scroll occurred.
    MouseWheel {
        delta: MouseScrollDelta,
        phase: TouchPhase,
        #[deprecated = "Deprecated in favor of WindowEvent::ModifiersChanged"]
        modifiers: ModifiersState,
    },

    /// An mouse button press has been received.
    MouseInput {
        state: ElementState,
        button: MouseButton,
        #[deprecated = "Deprecated in favor of WindowEvent::ModifiersChanged"]
        modifiers: ModifiersState,
    },

    /// Touchpad pressure event.
    ///
    /// At the moment, only supported on Apple forcetouch-capable macbooks.
    /// The parameters are: pressure level (value between 0 and 1 representing how hard the touchpad
    /// is being pressed) and stage (integer representing the click level).
    TouchpadPressure { pressure: f32, stage: i64 },

    /// Touch event has been received
    Touch(Touch),

    /// The window's scale factor has changed.
    ///
    /// The following user actions can cause DPI changes:
    ///
    /// * Changing the display's resolution.
    /// * Changing the display's scale factor (e.g. in Control Panel on Windows).
    /// * Moving the window to a display with a different scale factor.
    ///
    /// After this event callback has been processed, the window will be resized to whatever value
    /// is pointed to by the `new_inner_size` reference. By default, this will contain the size suggested
    /// by the OS, but it can be changed to any value.
    ///
    /// For more information about DPI in general, see the [`dpi`](crate::dpi) module.
    ScaleFactorChanged {
        scale_factor: f64,
        new_inner_size: &'a mut PhysicalSize<u32>,
    },

    /// The system window theme has changed.
    ///
    /// Applications might wish to react to this to change the theme of the content of the window
    /// when the system changes the window theme.
    ///
    /// At the moment this is only supported on Windows.
    ThemeChanged(Theme),
}

impl Clone for WindowEvent<'static> {
    fn clone(&self) -> Self {
        use self::WindowEvent::*;
        return match self {
            Resized(size) => Resized(size.clone()),
            Moved(pos) => Moved(pos.clone()),
            CloseRequested => CloseRequested,
            Destroyed => Destroyed,
            DroppedFile(file) => DroppedFile(file.clone()),
            HoveredFile(file) => HoveredFile(file.clone()),
            HoveredFileCancelled => HoveredFileCancelled,
            ReceivedCharacter(c) => ReceivedCharacter(*c),
            Focused(f) => Focused(*f),
            KeyboardInput(keyboard_input) => KeyboardInput(keyboard_input.clone()),
            ModifiersChanged(modifiers) => ModifiersChanged(modifiers.clone()),
            #[allow(deprecated)]
            CursorMoved {
                position,
                modifiers,
            } => CursorMoved {
                position: *position,
                modifiers: *modifiers,
            },
            CursorEntered => CursorEntered,
            CursorLeft => CursorLeft,
            #[allow(deprecated)]
            MouseWheel {
                delta,
                phase,
                modifiers,
            } => MouseWheel {
                delta: *delta,
                phase: *phase,
                modifiers: *modifiers,
            },
            #[allow(deprecated)]
            MouseInput {
                state,
                button,
                modifiers,
            } => MouseInput {
                state: *state,
                button: *button,
                modifiers: *modifiers,
            },
            TouchpadPressure { pressure, stage } => TouchpadPressure {
                pressure: *pressure,
                stage: *stage,
            },
            Touch(touch) => Touch(*touch),
            ThemeChanged(theme) => ThemeChanged(theme.clone()),
            ScaleFactorChanged { .. } => {
                unreachable!("Static event can't be about scale factor changing")
            }
        };
    }
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
            KeyboardInput(keyboard_input) => Some(KeyboardInput(keyboard_input)),
            ModifiersChanged(modifiers) => Some(ModifiersChanged(modifiers)),
            #[allow(deprecated)]
            CursorMoved {
                position,
                modifiers,
            } => Some(CursorMoved {
                position,
                modifiers,
            }),
            CursorEntered {} => Some(CursorEntered {}),
            CursorLeft {} => Some(CursorLeft {}),
            #[allow(deprecated)]
            MouseWheel {
                delta,
                phase,
                modifiers,
            } => Some(MouseWheel {
                delta,
                phase,
                modifiers,
            }),
            #[allow(deprecated)]
            MouseInput {
                state,
                button,
                modifiers,
            } => Some(MouseInput {
                state,
                button,
                modifiers,
            }),
            TouchpadPressure { pressure, stage } => Some(TouchpadPressure { pressure, stage }),
            Touch(touch) => Some(Touch(touch)),
            ThemeChanged(theme) => Some(ThemeChanged(theme)),
            ScaleFactorChanged { .. } => None,
        }
    }
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
    #[deprecated = "Deprecated in favor of WindowEvent::ModifiersChanged"]
    pub modifiers: ModifiersState,
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

/// Represents a touch event
///
/// Every time the user touches the screen, a new `Start` event with an unique
/// identifier for the finger is generated. When the finger is lifted, an `End`
/// event is generated with the same finger id.
///
/// After a `Start` event has been emitted, there may be zero or more `Move`
/// events when the finger is moved or the touch pressure changes.
///
/// The finger id may be reused by the system after an `End` event. The user
/// should assume that a new `Start` event received with the same id has nothing
/// to do with the old finger and is a new finger.
///
/// A `Cancelled` event is emitted when the system has canceled tracking this
/// touch, such as when the window loses focus, or on iOS if the user moves the
/// device against their face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    pub phase: TouchPhase,
    pub location: PhysicalPosition<f64>,
    /// Describes how hard the screen was pressed. May be `None` if the platform
    /// does not support pressure sensitivity.
    ///
    /// ## Platform-specific
    ///
    /// - Only available on **iOS** 9.0+ and **Windows** 8+.
    pub force: Option<Force>,
    /// Unique identifier of a finger.
    ///
    /// This may get reused by the system after the touch ends.
    pub id: u64,
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
    Other(u16),
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
    PixelDelta(PhysicalPosition<f64>),
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
    NumpadAdd,
    NumpadDivide,
    NumpadDecimal,
    NumpadComma,
    NumpadEnter,
    NumpadEquals,
    NumpadMultiply,
    NumpadSubtract,

    AbntC1,
    AbntC2,
    Apostrophe,
    Apps,
    Asterisk,
    At,
    Ax,
    Backslash,
    Calculator,
    Capital,
    Colon,
    Comma,
    Convert,
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
    Mute,
    MyComputer,
    // also called "Next"
    NavigateForward,
    // also called "Prior"
    NavigateBackward,
    NextTrack,
    NoConvert,
    OEM102,
    Period,
    PlayPause,
    Plus,
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
