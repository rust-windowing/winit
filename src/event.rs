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
use std::{fmt, path::PathBuf};

use crate::{
    dpi::{PhysicalDelta, PhysicalPosition, PhysicalSize, UnitlessDelta},
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
    WindowEvent(WindowId, WindowEvent<'a>),

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

    /// A file is being hovered over the window.
    ///
    /// When the user hovers multiple files at once, this event will be emitted for each file
    /// separately.
    FileHovered(PathBuf),

    /// A file has been dropped into the window.
    ///
    /// When the user drops multiple files at once, this event will be emitted for each file
    /// separately.
    FileDropped(PathBuf),

    /// A file was hovered, but has exited the window.
    ///
    /// There will be a single `HoveredFileCancelled` event triggered even if multiple files were
    /// hovered.
    FileHoverCancelled,

    /// The window gained focus.
    FocusGained,

    /// The window lost focus.
    FocusLost,

    Key(KeyEvent),

    /// The window received a unicode character.
    CharReceived(char),

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
    PointerForce(PointerId, Force),
    PointerTilt(PointerId, PointerTiltEvent),
    PointerTwist(PointerId, f64),
    PointerContactArea(PointerId, PhysicalSize<f64>),
    PointerMoved(PointerId, PhysicalPosition<f64>),
    PointerButton(PointerId, PointerButtonEvent),
    PointerEntered(PointerId),
    PointerLeft(PointerId),
    PointerDestroyed(PointerId),

    // TODO: SHOULD SCROLL EVENTS BE ASSOCIATED WITH A POINTER?
    ScrollStarted,
    ScrollLines(UnitlessDelta<f64>),
    ScrollPixels(PhysicalDelta<f64>),
    ScrollEnded,

    /// The system window theme has changed.
    ///
    /// Applications might wish to react to this to change the theme of the content of the window
    /// when the system changes the window theme.
    ///
    /// At the moment this is only supported on Windows.
    ThemeChanged(Theme),

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

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RawPointerEvent {
    /// A device has been added.
    Added,
    /// A device has been removed.
    Removed,
    Button(RawPointerButtonEvent),
    /// Relative change in physical position of a pointing device.
    ///
    /// This represents raw, unfiltered physical motion, NOT the position of the mouse. Accordingly,
    /// the values provided here are the change in position of the mouse since the previous
    /// `MovedRelative` event.
    MovedRelative(PhysicalDelta<f64>),
    /// Change in absolute position of a pointing device.
    ///
    /// The `PhysicalPosition` value is the new position of the cursor relative to the desktop. This
    /// generally doesn't get output by standard mouse devices, but can get output from tablet devices.
    MovedAbsolute(PhysicalPosition<f64>),
    /// Change in rotation of mouse wheel.
    Wheel(UnitlessDelta<f64>),
}

/// Raw keyboard events.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum RawKeyboardEvent {
    /// A keyboard device has been added.
    Added,
    /// A keyboard device has been removed.
    Removed,
    Key(RawKeyEvent),
}

impl<'a, T> Event<'a, T> {
    pub fn map_nonuser_event<U>(self) -> Result<Event<'a, U>, Event<'a, T>> {
        use self::Event::*;
        match self {
            UserEvent(_) => Err(self),
            WindowEvent(window_id, event) => Ok(WindowEvent(window_id, event)),
            RawPointerEvent(pointer_id, event) => Ok(RawPointerEvent(pointer_id, event)),
            RawKeyboardEvent(keyboard_id, event) => Ok(RawKeyboardEvent(keyboard_id, event)),
            AppEvent(app_event) => Ok(AppEvent(app_event)),
            NewEvents(cause) => Ok(NewEvents(cause)),
            MainEventsCleared => Ok(MainEventsCleared),
            RedrawRequested(wid) => Ok(RedrawRequested(wid)),
            RedrawEventsCleared => Ok(RedrawEventsCleared),
            LoopDestroyed => Ok(LoopDestroyed),
        }
    }

    /// If the event doesn't contain a reference, turn it into an event with a `'static` lifetime.
    /// Otherwise, return `None`.
    pub fn to_static(self) -> Result<Event<'static, T>, Event<'a, T>> {
        use self::Event::*;
        match self {
            NewEvents(cause) => Ok(NewEvents(cause)),
            WindowEvent(window_id, event) => event
                .to_static()
                .map(|e| -> Event<'static, T> { WindowEvent(window_id, e) })
                .map_err(|e| -> Event<'a, T> { WindowEvent(window_id, e) }),
            RawPointerEvent(pointer_id, event) => Ok(RawPointerEvent(pointer_id, event)),
            RawKeyboardEvent(keyboard_id, event) => Ok(RawKeyboardEvent(keyboard_id, event)),
            AppEvent(app_event) => Ok(AppEvent(app_event)),
            UserEvent(e) => Ok(UserEvent(e)),
            MainEventsCleared => Ok(MainEventsCleared),
            RedrawRequested(wid) => Ok(RedrawRequested(wid)),
            RedrawEventsCleared => Ok(RedrawEventsCleared),
            LoopDestroyed => Ok(LoopDestroyed),
        }
    }
}

impl Clone for WindowEvent<'static> {
    fn clone(&self) -> Self {
        use self::WindowEvent::*;
        match *self {
            Resized(size) => Resized(size),
            Moved(position) => Moved(position),
            CloseRequested => CloseRequested,
            Destroyed => Destroyed,
            FileDropped(ref path) => FileDropped(path.clone()),
            FileHovered(ref path) => FileHovered(path.clone()),
            FileHoverCancelled => FileHoverCancelled,
            FocusGained => FocusGained,
            FocusLost => FocusLost,
            CharReceived(char) => CharReceived(char),
            Key(key_press) => Key(key_press),
            ModifiersChanged(state) => ModifiersChanged(state),
            PointerCreated(id) => PointerCreated(id),
            PointerTilt(id, tilt) => PointerTilt(id, tilt),
            PointerTwist(id, twist) => PointerTwist(id, twist),
            PointerForce(id, force) => PointerForce(id, force),
            PointerContactArea(id, contact_area) => PointerContactArea(id, contact_area),
            PointerMoved(id, position) => PointerMoved(id, position),
            PointerButton(id, pointer_button) => PointerButton(id, pointer_button),
            PointerEntered(id) => PointerEntered(id),
            PointerLeft(id) => PointerLeft(id),
            PointerDestroyed(id) => PointerDestroyed(id),
            ScrollStarted => ScrollStarted,
            ScrollLines(delta) => ScrollLines(delta),
            ScrollPixels(delta) => ScrollPixels(delta),
            ScrollEnded => ScrollEnded,
            ThemeChanged(theme) => ThemeChanged(theme),
            ScaleFactorChanged(..) => {
                unreachable!("Static event can't be about scale factor changing")
            }
        }
    }
}

impl<'a> WindowEvent<'a> {
    pub fn to_static(self) -> Result<WindowEvent<'static>, WindowEvent<'a>> {
        use self::WindowEvent::*;
        match self {
            Resized(size) => Ok(Resized(size)),
            Moved(position) => Ok(Moved(position)),
            CloseRequested => Ok(CloseRequested),
            Destroyed => Ok(Destroyed),
            FileDropped(path) => Ok(FileDropped(path)),
            FileHovered(path) => Ok(FileHovered(path)),
            FileHoverCancelled => Ok(FileHoverCancelled),
            FocusGained => Ok(FocusGained),
            FocusLost => Ok(FocusLost),
            CharReceived(char) => Ok(CharReceived(char)),
            Key(key_press) => Ok(Key(key_press)),
            ModifiersChanged(state) => Ok(ModifiersChanged(state)),
            PointerCreated(id) => Ok(PointerCreated(id)),
            PointerTilt(id, tilt) => Ok(PointerTilt(id, tilt)),
            PointerTwist(id, twist) => Ok(PointerTwist(id, twist)),
            PointerForce(id, force) => Ok(PointerForce(id, force)),
            PointerContactArea(id, contact_area) => Ok(PointerContactArea(id, contact_area)),
            PointerMoved(id, position) => Ok(PointerMoved(id, position)),
            PointerButton(id, pointer_button) => Ok(PointerButton(id, pointer_button)),
            PointerEntered(id) => Ok(PointerEntered(id)),
            PointerLeft(id) => Ok(PointerLeft(id)),
            PointerDestroyed(id) => Ok(PointerDestroyed(id)),
            ScrollStarted => Ok(ScrollStarted),
            ScrollLines(delta) => Ok(ScrollLines(delta)),
            ScrollPixels(delta) => Ok(ScrollPixels(delta)),
            ScrollEnded => Ok(ScrollEnded),
            ThemeChanged(theme) => Ok(ThemeChanged(theme)),
            ScaleFactorChanged(..) => Err(self),
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

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyEvent {
    pub(crate) logical_key: Option<LogicalKey>,
    pub(crate) scan_code: u32,
    pub(crate) is_down: bool,
    pub(crate) is_repeat: bool,
    pub(crate) is_synthetic: bool,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RawKeyEvent {
    pub(crate) logical_key: Option<LogicalKey>,
    pub(crate) scan_code: u32,
    pub(crate) is_down: bool,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PointerButtonEvent {
    pub(crate) button: PointerButton,
    pub(crate) is_down: bool,
    pub(crate) click_count: u32,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RawPointerButtonEvent {
    pub(crate) button: PointerButton,
    pub(crate) is_down: bool,
}

impl KeyEvent {
    pub fn logical_key(&self) -> Option<LogicalKey> {
        self.logical_key
    }

    pub fn logical_key_is(&self, key: LogicalKey) -> bool {
        self.logical_key == Some(key)
    }

    pub fn scan_code(&self) -> u32 {
        self.scan_code
    }
    pub fn is_down(&self) -> bool {
        self.is_down
    }
    pub fn is_up(&self) -> bool {
        !self.is_down
    }
    /// Is `true` if the user has held down the key long enough to send duplicate events.
    ///
    /// Is always `false` if `is_down` is `false`.
    pub fn is_repeat(&self) -> bool {
        self.is_repeat
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

impl KeyEvent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_logical_key(&mut self, logical_key: Option<LogicalKey>) -> &mut Self {
        self.logical_key = logical_key;
        self
    }
    pub fn set_scan_code(&mut self, scan_code: u32) -> &mut Self {
        self.scan_code = scan_code;
        self
    }
    pub fn set_is_down(&mut self, is_down: bool) -> &mut Self {
        self.is_down = is_down;
        self
    }
    pub fn set_is_repeat(&mut self, is_repeat: bool) -> &mut Self {
        self.is_repeat = is_repeat;
        self
    }
    pub fn set_is_synthetic(&mut self, is_synthetic: bool) -> &mut Self {
        self.is_synthetic = is_synthetic;
        self
    }
}

impl RawKeyEvent {
    pub fn logical_key(&self) -> Option<LogicalKey> {
        self.logical_key
    }
    pub fn scan_code(&self) -> u32 {
        self.scan_code
    }
    pub fn is_down(&self) -> bool {
        self.is_down
    }
    pub fn is_up(&self) -> bool {
        !self.is_down
    }
}

impl RawKeyEvent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_logical_key(&mut self, logical_key: Option<LogicalKey>) -> &mut Self {
        self.logical_key = logical_key;
        self
    }
    pub fn set_scan_code(&mut self, scan_code: u32) -> &mut Self {
        self.scan_code = scan_code;
        self
    }
    pub fn set_is_down(&mut self, is_down: bool) -> &mut Self {
        self.is_down = is_down;
        self
    }
}

impl PointerButtonEvent {
    pub fn button(&self) -> PointerButton {
        self.button
    }
    pub fn is_down(&self) -> bool {
        self.is_down
    }
    pub fn is_up(&self) -> bool {
        !self.is_down
    }
    /// The number of clicks the user has made in the same spot within the system's double-click
    /// interval. `1` is emitted on the first click, `2` is emitted on the second click, etc.
    ///
    /// Is always `0` if `is_down` is `false`.
    pub fn click_count(&self) -> u32 {
        self.click_count
    }
}

impl PointerButtonEvent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_button(&mut self, button: PointerButton) -> &mut Self {
        self.button = button;
        self
    }
    pub fn set_is_down(&mut self, is_down: bool) -> &mut Self {
        self.is_down = is_down;
        self
    }
    pub fn set_click_count(&mut self, click_count: u32) -> &mut Self {
        self.click_count = click_count;
        self
    }
}

impl RawPointerButtonEvent {
    pub fn button(&self) -> PointerButton {
        self.button
    }
    pub fn is_down(&self) -> bool {
        self.is_down
    }
    pub fn is_up(&self) -> bool {
        !self.is_down
    }
}

impl RawPointerButtonEvent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_button(&mut self, button: PointerButton) -> &mut Self {
        self.button = button;
        self
    }
    pub fn set_is_down(&mut self, is_down: bool) -> &mut Self {
        self.is_down = is_down;
        self
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct PointerButton(PointerButtonInner);

/// We use an internal enum rather than exposing the variants directly so that the `BUTTON_n`
/// constants are formatted and exposed in a similar style to the `{MOUSE|TOUCH|PEN}_n` constants.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename = "PointerButton"))]
#[cfg_attr(feature = "serde", serde(try_from = "pointer_button_serde::PointerButtonSerialize"))]
#[cfg_attr(feature = "serde", serde(into = "pointer_button_serde::PointerButtonSerialize"))]
#[allow(non_camel_case_types)]
enum PointerButtonInner {
    BUTTON_1,
    BUTTON_2,
    BUTTON_3,
    BUTTON_4,
    BUTTON_5,
    BUTTON_6,
}

impl Default for PointerButtonInner {
    #[inline(always)]
    fn default() -> Self {
        PointerButtonInner::BUTTON_1
    }
}

impl PointerButton {
    pub const MOUSE_LEFT: Self = Self::BUTTON_1;
    pub const MOUSE_RIGHT: Self = Self::BUTTON_2;
    pub const MOUSE_MIDDLE: Self = Self::BUTTON_3;
    pub const MOUSE_X1: Self = Self::BUTTON_4;
    pub const MOUSE_X2: Self = Self::BUTTON_5;

    pub const TOUCH_CONTACT: Self = Self::BUTTON_1;

    pub const PEN_CONTACT: Self = Self::BUTTON_1;
    pub const PEN_BARREL: Self = Self::BUTTON_2;
    pub const PEN_ERASER: Self = Self::BUTTON_6;

    pub const BUTTON_1: Self = Self(PointerButtonInner::BUTTON_1);
    pub const BUTTON_2: Self = Self(PointerButtonInner::BUTTON_2);
    pub const BUTTON_3: Self = Self(PointerButtonInner::BUTTON_3);
    pub const BUTTON_4: Self = Self(PointerButtonInner::BUTTON_4);
    pub const BUTTON_5: Self = Self(PointerButtonInner::BUTTON_5);
    pub const BUTTON_6: Self = Self(PointerButtonInner::BUTTON_6);

    pub fn as_u8(&self) -> u8 {
        self.0 as u8
    }
    pub fn is_mouse_left(&self) -> bool {
        *self == Self::MOUSE_LEFT
    }
    pub fn is_mouse_right(&self) -> bool {
        *self == Self::MOUSE_RIGHT
    }
    pub fn is_mouse_middle(&self) -> bool {
        *self == Self::MOUSE_MIDDLE
    }
    pub fn is_mouse_x1(&self) -> bool {
        *self == Self::MOUSE_X1
    }
    pub fn is_mouse_x2(&self) -> bool {
        *self == Self::MOUSE_X2
    }
    pub fn is_touch_contact(&self) -> bool {
        *self == Self::TOUCH_CONTACT
    }
    pub fn is_pen_contact(&self) -> bool { *self == Self::PEN_CONTACT }
    pub fn is_pen_barrel(&self) -> bool { *self == Self::PEN_BARREL }
    pub fn is_pen_eraser(&self) -> bool { *self == Self::PEN_ERASER }
    pub fn is_button_1(&self) -> bool {
        *self == Self::BUTTON_1
    }
    pub fn is_button_2(&self) -> bool {
        *self == Self::BUTTON_2
    }
    pub fn is_button_3(&self) -> bool {
        *self == Self::BUTTON_3
    }
    pub fn is_button_4(&self) -> bool {
        *self == Self::BUTTON_4
    }
    pub fn is_button_5(&self) -> bool {
        *self == Self::BUTTON_5
    }
    pub fn is_button_6(&self) -> bool { *self == Self::BUTTON_6 }

    /// Serializes the `PointerButton` as the `BUTTON_*` constants. This is the default
    /// serialization style, since it's pointer-type agnostic.
    ///
    /// For use with `#[serde(serialize_with = "path")]`
    #[cfg(feature = "serde")]
    pub fn serialize_agnostic<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        serde::Serialize::serialize(&self.0.as_serialize_agnostic(), serializer)
    }

    /// Tries to serialize the `PointerButton` as the `MOUSE_*` constants, falling back to
    /// the `BUTTON_{NUM}` constants if the value doesn't map onto a mouse constant.
    ///
    /// For use with `#[serde(serialize_with = "path")]`
    #[cfg(feature = "serde")]
    pub fn serialize_mouse<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        serde::Serialize::serialize(&self.0.as_serialize_mouse(), serializer)
    }

    /// Tries to serialize the `PointerButton` as the `TOUCH_*` constants, falling back to
    /// the `BUTTON_{NUM}` constants if the value doesn't map onto a touch constant.
    ///
    /// For use with `#[serde(serialize_with = "path")]`
    #[cfg(feature = "serde")]
    pub fn serialize_touch<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        serde::Serialize::serialize(&self.0.as_serialize_touch(), serializer)
    }

    /// Tries to serialize the `PointerButton` as the `PEN_*` constants, falling back to
    /// the `BUTTON_{NUM}` constants if the value doesn't map onto a pen constant.
    ///
    /// For use with `#[serde(serialize_with = "path")]`
    #[cfg(feature = "serde")]
    pub fn serialize_pen<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        serde::Serialize::serialize(&self.0.as_serialize_pen(), serializer)
    }

    /// Serializes the `PointerButton` as a legacy Winit [`MouseButton`]. This is provided for
    /// backwards-compatibility purposes.
    ///
    /// For use with `#[serde(serialize_with = "path")]`
    ///
    /// [`MouseButton`]: https://docs.rs/winit/0.22.2/winit/event/enum.MouseButton.html
    #[cfg(feature = "serde")]
    pub fn serialize_legacy<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        serde::Serialize::serialize(&self.0.as_serialize_legacy(), serializer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PointerId {
    MouseId(MouseId),
    TouchId(TouchId),
    PenId(PenId),
}

/// A typed identifier for a pointer device.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MouseId(pub(crate) platform_impl::MouseId);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TouchId(pub(crate) platform_impl::TouchId);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PenId(pub(crate) platform_impl::PenId);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointerDeviceId(pub(crate) platform_impl::PointerDeviceId);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyboardDeviceId(pub(crate) platform_impl::KeyboardDeviceId);

impl PointerId {
    pub fn is_mouse_id(&self) -> bool {
        match *self {
            PointerId::MouseId(_) => true,
            _ => false,
        }
    }

    pub fn is_touch_id(&self) -> bool {
        match *self {
            PointerId::TouchId(_) => true,
            _ => false,
        }
    }

    pub fn is_pen_id(&self) -> bool {
        match *self {
            PointerId::PenId(_) => true,
            _ => false,
        }
    }
}

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

impl fmt::Debug for PenId {
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerTiltEvent {
    tilt_angle_x: f64,
    tilt_angle_y: f64,
}

impl PointerTiltEvent {
    pub fn new_tilt_angle(tilt_angle_x: f64, tilt_angle_y: f64) -> PointerTiltEvent {
        PointerTiltEvent {
            tilt_angle_x,
            tilt_angle_y,
        }
    }

    #[inline(always)]
    pub fn tilt_angle_x(&self) -> f64 {
        self.tilt_angle_x
    }

    #[inline(always)]
    pub fn tilt_angle_y(&self) -> f64 {
        self.tilt_angle_y
    }

    #[inline(always)]
    pub fn tilt_angle(&self) -> f64 {
        // As shown by `tilt_vector_(x|y)`, the length of the tilt vector on the XY plane is
        // trivially relatable to the tilt angle. This computes the length of the XY vector and uses
        // that to get the axis-independent tilt angle.
        let tilt_xy_distance_from_center = f64::sqrt(self.tilt_vector_x().powi(2) + self.tilt_vector_y().powi(2));
        let tilt = f64::asin(tilt_xy_distance_from_center);
        if tilt.is_nan() {
            0.0
        } else {
            tilt
        }
    }

    #[inline(always)]
    pub fn altitude_angle(&self) -> f64 {
        std::f64::consts::PI/2.0 - self.tilt_angle()
    }

    #[inline(always)]
    pub fn tilt_vector_x(&self) -> f64 {
        self.tilt_angle_x.sin()
    }

    #[inline(always)]
    pub fn tilt_vector_y(&self) -> f64 {
        self.tilt_angle_y.sin()
    }

    #[inline(always)]
    pub fn tilt_vector_z(&self) -> f64 {
        // The tilt vector is a normalized three-component vector. Since we know the X and Y
        // components of that vector, we can use a transformed version of the pythagorean theorem
        // to get the Z component.
        let z = f64::sqrt(1.0 - self.tilt_vector_x().powi(2) - self.tilt_vector_y().powi(2));
        if z.is_nan() {
            0.0
        } else {
            z
        }
    }

    #[inline(always)]
    pub fn azimuth_angle(&self) -> Option<f64> {
        if self.tilt_angle_x == 0.0 && self.tilt_angle_y == 0.0 {
            None
        } else {
            Some(f64::atan2(self.tilt_angle_x, self.tilt_angle_y))
        }
    }

    #[inline(always)]
    pub fn azimuth_vector_x(&self) -> f64 {
        self.azimuth_angle().map(|a| a.sin()).unwrap_or(0.0)
    }

    #[inline(always)]
    pub fn azimuth_vector_y(&self) -> f64 {
        self.azimuth_angle().map(|a| a.cos()).unwrap_or(0.0)
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

#[cfg(feature = "serde")]
mod pointer_button_serde {
    use super::PointerButtonInner;
    use std::{
        convert::TryFrom,
        fmt,
    };
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
    #[derive(Serialize, Deserialize)]
    #[serde(rename = "PointerButton")]
    #[allow(non_camel_case_types)]
    pub enum PointerButtonSerialize {
        // legacy style
        Left,
        Right,
        Middle,
        Other(u8),

        // input-agnostic style
        BUTTON_1,
        BUTTON_2,
        BUTTON_3,
        BUTTON_4,
        BUTTON_5,
        BUTTON_6,

        // mouse style
        MOUSE_LEFT,
        MOUSE_RIGHT,
        MOUSE_MIDDLE,
        MOUSE_X1,
        MOUSE_X2,

        // touch style
        TOUCH_CONTACT,

        // pen style
        PEN_CONTACT,
        PEN_BARREL,
        PEN_ERASER,
    }


    pub struct OtherConvertError(u8);

    impl TryFrom<PointerButtonSerialize> for PointerButtonInner {
        type Error = OtherConvertError;
        fn try_from(serialize: PointerButtonSerialize) -> Result<PointerButtonInner, Self::Error> {
            match serialize {
                PointerButtonSerialize::TOUCH_CONTACT |
                PointerButtonSerialize::PEN_CONTACT |
                PointerButtonSerialize::BUTTON_1 |
                PointerButtonSerialize::Left |
                PointerButtonSerialize::MOUSE_LEFT => Ok(PointerButtonInner::BUTTON_1),

                PointerButtonSerialize::BUTTON_2 |
                PointerButtonSerialize::PEN_BARREL |
                PointerButtonSerialize::Right |
                PointerButtonSerialize::MOUSE_RIGHT => Ok(PointerButtonInner::BUTTON_2),

                PointerButtonSerialize::BUTTON_3 |
                PointerButtonSerialize::Middle |
                PointerButtonSerialize::MOUSE_MIDDLE => Ok(PointerButtonInner::BUTTON_3),

                PointerButtonSerialize::BUTTON_4 |
                PointerButtonSerialize::Other(0) |
                PointerButtonSerialize::MOUSE_X1 => Ok(PointerButtonInner::BUTTON_4),

                PointerButtonSerialize::BUTTON_5 |
                PointerButtonSerialize::Other(1) |
                PointerButtonSerialize::MOUSE_X2 => Ok(PointerButtonInner::BUTTON_5),

                PointerButtonSerialize::BUTTON_6 |
                PointerButtonSerialize::Other(2) |
                PointerButtonSerialize::PEN_ERASER => Ok(PointerButtonInner::BUTTON_6),

                PointerButtonSerialize::Other(i) => Err(OtherConvertError(i)),
            }
        }
    }

    impl From<PointerButtonInner> for PointerButtonSerialize {
        fn from(inner: PointerButtonInner) -> PointerButtonSerialize {
            inner.as_serialize_agnostic()
        }
    }

    impl PointerButtonInner {
        pub fn as_serialize_agnostic(&self) -> PointerButtonSerialize {
            match self {
                PointerButtonInner::BUTTON_1 => PointerButtonSerialize::BUTTON_1,
                PointerButtonInner::BUTTON_2 => PointerButtonSerialize::BUTTON_2,
                PointerButtonInner::BUTTON_3 => PointerButtonSerialize::BUTTON_3,
                PointerButtonInner::BUTTON_4 => PointerButtonSerialize::BUTTON_4,
                PointerButtonInner::BUTTON_5 => PointerButtonSerialize::BUTTON_5,
                PointerButtonInner::BUTTON_6 => PointerButtonSerialize::BUTTON_6,
            }
        }

        pub fn as_serialize_legacy(&self) -> PointerButtonSerialize {
            match self {
                PointerButtonInner::BUTTON_1 => PointerButtonSerialize::Left,
                PointerButtonInner::BUTTON_2 => PointerButtonSerialize::Right,
                PointerButtonInner::BUTTON_3 => PointerButtonSerialize::Middle,
                PointerButtonInner::BUTTON_4 => PointerButtonSerialize::Other(0),
                PointerButtonInner::BUTTON_5 => PointerButtonSerialize::Other(1),
                PointerButtonInner::BUTTON_6 => PointerButtonSerialize::BUTTON_6,
            }
        }

        pub fn as_serialize_mouse(&self) -> PointerButtonSerialize {
            match self {
                PointerButtonInner::BUTTON_1 => PointerButtonSerialize::MOUSE_LEFT,
                PointerButtonInner::BUTTON_2 => PointerButtonSerialize::MOUSE_RIGHT,
                PointerButtonInner::BUTTON_3 => PointerButtonSerialize::MOUSE_MIDDLE,
                PointerButtonInner::BUTTON_4 => PointerButtonSerialize::MOUSE_X1,
                PointerButtonInner::BUTTON_5 => PointerButtonSerialize::MOUSE_X2,
                PointerButtonInner::BUTTON_6 => PointerButtonSerialize::BUTTON_6,
            }
        }

        pub fn as_serialize_touch(&self) -> PointerButtonSerialize {
            match self {
                PointerButtonInner::BUTTON_1 => PointerButtonSerialize::TOUCH_CONTACT,
                PointerButtonInner::BUTTON_2 => PointerButtonSerialize::BUTTON_2,
                PointerButtonInner::BUTTON_3 => PointerButtonSerialize::BUTTON_3,
                PointerButtonInner::BUTTON_4 => PointerButtonSerialize::BUTTON_4,
                PointerButtonInner::BUTTON_5 => PointerButtonSerialize::BUTTON_5,
                PointerButtonInner::BUTTON_6 => PointerButtonSerialize::BUTTON_6,
            }
        }

        pub fn as_serialize_pen(&self) -> PointerButtonSerialize {
            match self {
                PointerButtonInner::BUTTON_1 => PointerButtonSerialize::PEN_CONTACT,
                PointerButtonInner::BUTTON_2 => PointerButtonSerialize::PEN_BARREL,
                PointerButtonInner::BUTTON_3 => PointerButtonSerialize::BUTTON_3,
                PointerButtonInner::BUTTON_4 => PointerButtonSerialize::BUTTON_4,
                PointerButtonInner::BUTTON_5 => PointerButtonSerialize::BUTTON_5,
                PointerButtonInner::BUTTON_6 => PointerButtonSerialize::PEN_ERASER,
            }
        }
    }

    impl fmt::Display for OtherConvertError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "could not deserialize Other({})", self.0)
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::event::PointerButton;
        use serde::{Serialize, Deserialize};

        /// legacy mouse button struct
        #[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
        #[derive(Serialize, Deserialize)]
        enum MouseButton {
            Left,
            Right,
            Middle,
            Other(u8),
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        #[serde(rename = "Serde")]
        struct LegacySerde(MouseButton);

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
        #[serde(rename = "Serde")]
        struct NewSerde(#[serde(serialize_with = "PointerButton::serialize_legacy")] PointerButton);

        trait Serde {
            type Error: std::fmt::Debug;
            fn to_bytes<T>(value: &T) -> Result<Vec<u8>, Self::Error>
            where
                T: Serialize;
            fn from_bytes<'a, T>(s: &'a [u8]) -> Result<T, Self::Error>
            where
                T: Deserialize<'a>;
        }

        struct Ron;
        impl Serde for Ron {
            type Error = ron::Error;
            fn to_bytes<T>(value: &T) -> Result<Vec<u8>, Self::Error>
            where
                T: Serialize
            {
                ron::ser::to_string(value).map(|s| s.into_bytes())
            }
            fn from_bytes<'a, T>(s: &'a [u8]) -> Result<T, Self::Error>
            where
                T: Deserialize<'a>
            {
                ron::de::from_bytes(s)
            }
        }

        struct Bincode;
        impl Serde for Bincode {
            type Error = bincode::Error;
            fn to_bytes<T>(value: &T) -> Result<Vec<u8>, Self::Error>
            where
                T: Serialize
            {
                bincode::serialize(value)
            }
            fn from_bytes<'a, T>(s: &'a [u8]) -> Result<T, Self::Error>
            where
                T: Deserialize<'a>
            {
                bincode::deserialize(s)
            }
        }

        struct Json;
        impl Serde for Json {
            type Error = serde_json::Error;
            fn to_bytes<T>(value: &T) -> Result<Vec<u8>, Self::Error>
            where
                T: Serialize
            {
                serde_json::to_vec(value)
            }
            fn from_bytes<'a, T>(s: &'a [u8]) -> Result<T, Self::Error>
            where
                T: Deserialize<'a>
            {
                serde_json::from_slice(s)
            }
        }

        fn serde<S: Serde>() {
            let legacy = [
                LegacySerde(MouseButton::Left),
                LegacySerde(MouseButton::Right),
                LegacySerde(MouseButton::Middle),
                LegacySerde(MouseButton::Other(0)),
                LegacySerde(MouseButton::Other(1)),
            ];
            let new = [
                NewSerde(PointerButton::MOUSE_LEFT),
                NewSerde(PointerButton::MOUSE_RIGHT),
                NewSerde(PointerButton::MOUSE_MIDDLE),
                NewSerde(PointerButton::MOUSE_X1),
                NewSerde(PointerButton::MOUSE_X2),
            ];

            let try_to_utf8 = |b: &[u8]| format!("\n\tstr: {}\n\tbytes: {:?}", std::str::from_utf8(b).unwrap_or_else(|_| ""), b);
            for (l, n) in legacy.iter().cloned().zip(new.iter().cloned()) {
                println!("legacy: {:?}, new: {:?}", l, n);
                let legacy_serialized: Vec<u8> = S::to_bytes(&l).unwrap();
                let new_serialized: Vec<u8> = S::to_bytes(&n).unwrap();
                println!("legacy serialized: {}", try_to_utf8(&legacy_serialized));
                println!("new serialized: {}", try_to_utf8(&new_serialized));

                let legacy_deserialized: LegacySerde = S::from_bytes(&new_serialized).unwrap();
                let new_deserialized: NewSerde = S::from_bytes(&legacy_serialized).unwrap();

                assert_eq!(&legacy_serialized, &new_serialized);
                assert_eq!(legacy_deserialized, l);
                assert_eq!(new_deserialized, n);
            }
        }

        #[test]
        fn serde_pointer_button_backwards_compatibility_ron() {
            serde::<Ron>();
        }

        #[test]
        fn serde_pointer_button_backwards_compatibility_bincode() {
            serde::<Bincode>();
        }

        #[test]
        fn serde_pointer_button_backwards_compatibility_json() {
            serde::<Json>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PointerTiltEvent;
    use std::f64::consts::PI;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn pointer_tilt_azimuth() {
        let half_angle = 0.707107;

        println!("up");
        let up = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: 0.0,
        };
        assert_eq!(None, up.azimuth_angle());

        println!("north");
        let north = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: PI / 2.0,
        };
        assert_approx_eq!(0.0, north.azimuth_angle().unwrap());
        assert_approx_eq!(0.0, north.azimuth_vector_x());
        assert_approx_eq!(1.0, north.azimuth_vector_y());

        println!("north_east");
        let north_east = PointerTiltEvent {
            tilt_angle_x: PI / 4.0,
            tilt_angle_y: PI / 4.0,
        };
        assert_approx_eq!(PI / 4.0, north_east.azimuth_angle().unwrap());
        assert_approx_eq!(half_angle, north_east.azimuth_vector_x());
        assert_approx_eq!(half_angle, north_east.azimuth_vector_y());

        println!("east");
        let east = PointerTiltEvent {
            tilt_angle_x: PI / 2.0,
            tilt_angle_y: 0.0,
        };
        assert_approx_eq!(1.0 * PI / 2.0, east.azimuth_angle().unwrap());
        assert_approx_eq!(1.0, east.azimuth_vector_x());
        assert_approx_eq!(0.0, east.azimuth_vector_y());

        println!("south_east");
        let south_east = PointerTiltEvent {
            tilt_angle_x: PI / 4.0,
            tilt_angle_y: -PI / 4.0,
        };
        assert_approx_eq!(3.0 * PI / 4.0, south_east.azimuth_angle().unwrap());
        assert_approx_eq!(half_angle, south_east.azimuth_vector_x());
        assert_approx_eq!(-half_angle, south_east.azimuth_vector_y());

        println!("south");
        let south = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: -PI / 2.0,
        };
        assert_approx_eq!(2.0 * PI / 2.0, south.azimuth_angle().unwrap());
        assert_approx_eq!(0.0, south.azimuth_vector_x());
        assert_approx_eq!(-1.0, south.azimuth_vector_y());

        println!("south_west");
        let south_west = PointerTiltEvent {
            tilt_angle_x: -PI / 4.0,
            tilt_angle_y: -PI / 4.0,
        };
        assert_approx_eq!(-3.0 * PI / 4.0, south_west.azimuth_angle().unwrap());
        assert_approx_eq!(-half_angle, south_west.azimuth_vector_x());
        assert_approx_eq!(-half_angle, south_west.azimuth_vector_y());

        println!("west");
        let west = PointerTiltEvent {
            tilt_angle_x: -PI / 2.0,
            tilt_angle_y: 0.0,
        };
        assert_approx_eq!(-1.0 * PI / 2.0, west.azimuth_angle().unwrap());
        assert_approx_eq!(-1.0, west.azimuth_vector_x());
        assert_approx_eq!(0.0, west.azimuth_vector_y());

        println!("north_west");
        let north_west = PointerTiltEvent {
            tilt_angle_x: -PI / 4.0,
            tilt_angle_y: PI / 4.0,
        };
        assert_approx_eq!(-PI / 4.0, north_west.azimuth_angle().unwrap());
        assert_approx_eq!(-half_angle, north_west.azimuth_vector_x());
        assert_approx_eq!(half_angle, north_west.azimuth_vector_y());
    }

    #[test]
    fn pointer_tilt_vector() {
        let tilt_vector = |te: PointerTiltEvent| [te.tilt_vector_x(), te.tilt_vector_y(), te.tilt_vector_z()];

        let eps = 1.0e-6;
        let assert_normalized = |v: [f64; 3]| {
            let length = (v[0].powi(2) + v[1].powi(2) + v[2].powi(2)).sqrt();
            if (length - 1.0).abs() > eps {
                assert_eq!(length, 1.0, "vector {:?} is not normalized", v);
            }
        };
        let assert_approx_eq = |left: [f64; 3], right: [f64; 3]| {
            println!("testing left normalized");
            assert_normalized(left);
            println!("testing right normalized");
            assert_normalized(right);

            let mut equals = true;
            for i in 0..3 {
                equals &= (left[i] - right[i]).abs() <= eps;
            }
            if !equals {
                assert_eq!(left, right);
            }
        };

        println!("up");
        let up = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: 0.0,
        };
        assert_approx_eq([0.0, 0.0, 1.0], tilt_vector(up));

        println!("east");
        let east = PointerTiltEvent {
            tilt_angle_x: PI / 2.0,
            tilt_angle_y: 0.0,
        };
        assert_approx_eq([1.0, 0.0, 0.0], tilt_vector(east));

        println!("west");
        let west = PointerTiltEvent {
            tilt_angle_x: -PI / 2.0,
            tilt_angle_y: 0.0,
        };
        assert_approx_eq([-1.0, 0.0, 0.0], tilt_vector(west));

        println!("north");
        let north = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: PI / 2.0,
        };
        assert_approx_eq([0.0, 1.0, 0.0], tilt_vector(north));

        println!("south");
        let south = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: -PI / 2.0,
        };
        assert_approx_eq([0.0, -1.0, 0.0], tilt_vector(south));

        let half_angle = 0.707107;
        let circle_corner = PI/4.0;
        println!("north_east");
        let north_east = PointerTiltEvent {
            tilt_angle_x: circle_corner,
            tilt_angle_y: circle_corner,
        };
        assert_approx_eq([half_angle, half_angle, 0.0], tilt_vector(north_east));

        println!("half_east");
        let half_east = PointerTiltEvent {
            tilt_angle_x: PI / 4.0,
            tilt_angle_y: 0.0,
        };
        assert_approx_eq([half_angle, 0.0, half_angle], tilt_vector(half_east));

        println!("half_north");
        let half_north = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: PI / 4.0,
        };
        assert_approx_eq([0.0, half_angle, half_angle], tilt_vector(half_north));

        println!("half_north_east");
        let half_north_east = PointerTiltEvent {
            tilt_angle_x: PI / 6.0,
            tilt_angle_y: PI / 6.0,
        };
        assert_approx_eq([0.5, 0.5, half_angle], tilt_vector(half_north_east));
    }

    #[test]
    fn pointer_tilt_angle() {
        let angle_slight = PI / 6.0;
        let angle_lots = PI / 3.0;
        let angle_full = PI / 2.0;
        let diagonal_angle = |a: f64| (a.sin().powi(2)/2.0).sqrt().asin();
        let diagonal_slight = diagonal_angle(angle_slight);
        let diagonal_lots = diagonal_angle(angle_lots);
        let diagonal_full = diagonal_angle(angle_full);

        println!("up");
        let up = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: 0.0,
        };
        assert_approx_eq!(0.0, up.tilt_angle());

        println!("north_slight");
        let north_slight = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: angle_slight,
        };
        assert_approx_eq!(angle_slight, north_slight.tilt_angle());

        println!("north_lots");
        let north_lots = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: angle_lots,
        };
        assert_approx_eq!(angle_lots, north_lots.tilt_angle());

        println!("north_full");
        let north_full = PointerTiltEvent {
            tilt_angle_x: 0.0,
            tilt_angle_y: angle_full,
        };
        assert_approx_eq!(angle_full, north_full.tilt_angle());

        println!("north_east_slight");
        let north_east_slight = PointerTiltEvent {
            tilt_angle_x: diagonal_slight,
            tilt_angle_y: diagonal_slight,
        };
        assert_approx_eq!(angle_slight, north_east_slight.tilt_angle());

        println!("north_east_lots");
        let north_east_lots = PointerTiltEvent {
            tilt_angle_x: -diagonal_lots,
            tilt_angle_y: -diagonal_lots,
        };
        assert_approx_eq!(angle_lots, north_east_lots.tilt_angle());

        println!("south_east_full");
        let south_east_full = PointerTiltEvent {
            tilt_angle_x: -diagonal_full,
            tilt_angle_y: -diagonal_full,
        };
        assert_approx_eq!(angle_full, south_east_full.tilt_angle());


        println!("south_slight");
        let south_slight = PointerTiltEvent {
            tilt_angle_x: -0.0,
            tilt_angle_y: -angle_slight,
        };
        assert_approx_eq!(angle_slight, south_slight.tilt_angle());

        println!("south_lots");
        let south_lots = PointerTiltEvent {
            tilt_angle_x: -0.0,
            tilt_angle_y: -angle_lots,
        };
        assert_approx_eq!(angle_lots, south_lots.tilt_angle());

        println!("south_full");
        let south_full = PointerTiltEvent {
            tilt_angle_x: -0.0,
            tilt_angle_y: -angle_full,
        };
        assert_approx_eq!(angle_full, south_full.tilt_angle());

        println!("south_west_slight");
        let south_west_slight = PointerTiltEvent {
            tilt_angle_x: -diagonal_slight,
            tilt_angle_y: -diagonal_slight,
        };
        assert_approx_eq!(angle_slight, south_west_slight.tilt_angle());

        println!("south_west_lots");
        let south_west_lots = PointerTiltEvent {
            tilt_angle_x: -diagonal_lots,
            tilt_angle_y: -diagonal_lots,
        };
        assert_approx_eq!(angle_lots, south_west_lots.tilt_angle());

        println!("south_west_full");
        let south_west_full = PointerTiltEvent {
            tilt_angle_x: -diagonal_full,
            tilt_angle_y: -diagonal_full,
        };
        assert_approx_eq!(angle_full, south_west_full.tilt_angle());
    }
}
