//! The [`Event`] enum and assorted supporting types.
//!
//! These are sent to the closure given to [`EventLoop::run(...)`], where they get
//! processed and used to modify the program state. For more details, see the root-level documentation.
//!
//! Some of these events represent different "parts" of a traditional event-handling loop. You could
//! approximate the basic ordering loop of [`EventLoop::run(...)`] like this:
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
//! This leaves out timing details like [`ControlFlow::WaitUntil`] but hopefully
//! describes what happens in what order.
//!
//! [`EventLoop::run(...)`]: crate::event_loop::EventLoop::run
//! [`ControlFlow::WaitUntil`]: crate::event_loop::ControlFlow::WaitUntil
use instant::Instant;
use std::path::PathBuf;

#[cfg(doc)]
use crate::window::Window;
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

    /// Emitted when an event is sent from [`EventLoopProxy::send_event`](crate::event_loop::EventLoopProxy::send_event)
    UserEvent(T),

    /// Emitted when the application has been suspended.
    ///
    /// # Portability
    ///
    /// Not all platforms support the notion of suspending applications, and there may be no
    /// technical way to guarantee being able to emit a `Suspended` event if the OS has
    /// no formal application lifecycle (currently only Android and iOS do). For this reason,
    /// Winit does not currently try to emit pseudo `Suspended` events before the application
    /// quits on platforms without an application lifecycle.
    ///
    /// Considering that the implementation of `Suspended` and [`Resumed`] events may be internally
    /// driven by multiple platform-specific events, and that there may be subtle differences across
    /// platforms with how these internal events are delivered, it's recommended that applications
    /// be able to gracefully handle redundant (i.e. back-to-back) `Suspended` or [`Resumed`] events.
    ///
    /// Also see [`Resumed`] notes.
    ///
    /// ## Android
    ///
    /// On Android, the `Suspended` event is only sent when the application's associated
    /// [`SurfaceView`] is destroyed. This is expected to closely correlate with the [`onPause`]
    /// lifecycle event but there may technically be a discrepancy.
    ///
    /// [`onPause`]: https://developer.android.com/reference/android/app/Activity#onPause()
    ///
    /// Applications that need to run on Android should assume their [`SurfaceView`] has been
    /// destroyed, which indirectly invalidates any existing render surfaces that may have been
    /// created outside of Winit (such as an `EGLSurface`, [`VkSurfaceKHR`] or [`wgpu::Surface`]).
    ///
    /// After being `Suspended` on Android applications must drop all render surfaces before
    /// the event callback completes, which may be re-created when the application is next [`Resumed`].
    ///
    /// [`SurfaceView`]: https://developer.android.com/reference/android/view/SurfaceView
    /// [Activity lifecycle]: https://developer.android.com/guide/components/activities/activity-lifecycle
    /// [`VkSurfaceKHR`]: https://www.khronos.org/registry/vulkan/specs/1.3-extensions/man/html/VkSurfaceKHR.html
    /// [`wgpu::Surface`]: https://docs.rs/wgpu/latest/wgpu/struct.Surface.html
    ///
    /// ## iOS
    ///
    /// On iOS, the `Suspended` event is currently emitted in response to an
    /// [`applicationWillResignActive`] callback which means that the application is
    /// about to transition from the active to inactive state (according to the
    /// [iOS application lifecycle]).
    ///
    /// [`applicationWillResignActive`]: https://developer.apple.com/documentation/uikit/uiapplicationdelegate/1622950-applicationwillresignactive
    /// [iOS application lifecycle]: https://developer.apple.com/documentation/uikit/app_and_environment/managing_your_app_s_life_cycle
    ///
    /// [`Resumed`]: Self::Resumed
    Suspended,

    /// Emitted when the application has been resumed.
    ///
    /// For consistency, all platforms emit a `Resumed` event even if they don't themselves have a
    /// formal suspend/resume lifecycle. For systems without a standard suspend/resume lifecycle
    /// the `Resumed` event is always emitted after the [`NewEvents(StartCause::Init)`][StartCause::Init]
    /// event.
    ///
    /// # Portability
    ///
    /// It's recommended that applications should only initialize their graphics context and create
    /// a window after they have received their first `Resumed` event. Some systems
    /// (specifically Android) won't allow applications to create a render surface until they are
    /// resumed.
    ///
    /// Considering that the implementation of [`Suspended`] and `Resumed` events may be internally
    /// driven by multiple platform-specific events, and that there may be subtle differences across
    /// platforms with how these internal events are delivered, it's recommended that applications
    /// be able to gracefully handle redundant (i.e. back-to-back) [`Suspended`] or `Resumed` events.
    ///
    /// Also see [`Suspended`] notes.
    ///
    /// ## Android
    ///
    /// On Android, the `Resumed` event is sent when a new [`SurfaceView`] has been created. This is
    /// expected to closely correlate with the [`onResume`] lifecycle event but there may technically
    /// be a discrepancy.
    ///
    /// [`onResume`]: https://developer.android.com/reference/android/app/Activity#onResume()
    ///
    /// Applications that need to run on Android must wait until they have been `Resumed`
    /// before they will be able to create a render surface (such as an `EGLSurface`,
    /// [`VkSurfaceKHR`] or [`wgpu::Surface`]) which depend on having a
    /// [`SurfaceView`]. Applications must also assume that if they are [`Suspended`], then their
    /// render surfaces are invalid and should be dropped.
    ///
    /// Also see [`Suspended`] notes.
    ///
    /// [`SurfaceView`]: https://developer.android.com/reference/android/view/SurfaceView
    /// [Activity lifecycle]: https://developer.android.com/guide/components/activities/activity-lifecycle
    /// [`VkSurfaceKHR`]: https://www.khronos.org/registry/vulkan/specs/1.3-extensions/man/html/VkSurfaceKHR.html
    /// [`wgpu::Surface`]: https://docs.rs/wgpu/latest/wgpu/struct.Surface.html
    ///
    /// ## iOS
    ///
    /// On iOS, the `Resumed` event is emitted in response to an [`applicationDidBecomeActive`]
    /// callback which means the application is "active" (according to the
    /// [iOS application lifecycle]).
    ///
    /// [`applicationDidBecomeActive`]: https://developer.apple.com/documentation/uikit/uiapplicationdelegate/1622956-applicationdidbecomeactive
    /// [iOS application lifecycle]: https://developer.apple.com/documentation/uikit/app_and_environment/managing_your_app_s_life_cycle
    ///
    /// [`Suspended`]: Self::Suspended
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

    /// Emitted after [`MainEventsCleared`] when a window should be redrawn.
    ///
    /// This gets triggered in two scenarios:
    /// - The OS has performed an operation that's invalidated the window's contents (such as
    ///   resizing the window).
    /// - The application has explicitly requested a redraw via [`Window::request_redraw`].
    ///
    /// During each iteration of the event loop, Winit will aggregate duplicate redraw requests
    /// into a single event, to help avoid duplicating rendering work.
    ///
    /// Mainly of interest to applications with mostly-static graphics that avoid redrawing unless
    /// something changes, like most non-game GUIs.
    ///
    ///
    /// ## Platform-specific
    ///
    /// - **macOS / iOS:** Due to implementation difficulties, this will often, but not always, be
    ///   emitted directly inside `drawRect:`, with neither a preceding [`MainEventsCleared`] nor
    ///   subsequent `RedrawEventsCleared`. See [#2640] for work on this.
    ///
    /// [`MainEventsCleared`]: Self::MainEventsCleared
    /// [`RedrawEventsCleared`]: Self::RedrawEventsCleared
    /// [#2640]: https://github.com/rust-windowing/winit/issues/2640
    RedrawRequested(WindowId),

    /// Emitted after all [`RedrawRequested`] events have been processed and control flow is about to
    /// be taken away from the program. If there are no `RedrawRequested` events, it is emitted
    /// immediately after `MainEventsCleared`.
    ///
    /// This event is useful for doing any cleanup or bookkeeping work after all the rendering
    /// tasks have been completed.
    ///
    /// [`RedrawRequested`]: Self::RedrawRequested
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
            DeviceEvent { device_id, event } => DeviceEvent {
                device_id: *device_id,
                event: event.clone(),
            },
            NewEvents(cause) => NewEvents(*cause),
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
            DeviceEvent { device_id, event } => Ok(DeviceEvent { device_id, event }),
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
            DeviceEvent { device_id, event } => Some(DeviceEvent { device_id, event }),
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

/// Describes the reason the event loop is resuming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartCause {
    /// Sent if the time specified by [`ControlFlow::WaitUntil`] has been reached. Contains the
    /// moment the timeout was requested and the requested resume time. The actual resume time is
    /// guaranteed to be equal to or after the requested resume time.
    ///
    /// [`ControlFlow::WaitUntil`]: crate::event_loop::ControlFlow::WaitUntil
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
    /// [`ControlFlow::Poll`].
    ///
    /// [`ControlFlow::Poll`]: crate::event_loop::ControlFlow::Poll
    Poll,

    /// Sent once, immediately after `run` is called. Indicates that the loop was just initialized.
    Init,
}

/// Describes an event from a [`Window`].
#[derive(Debug, PartialEq)]
pub enum WindowEvent<'a> {
    /// The size of the window has changed. Contains the client area's new dimensions.
    Resized(PhysicalSize<u32>),

    /// The position of the window has changed. Contains the window's new position.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web / Wayland:** Unsupported.
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
    ///
    /// See also the [`Ime`](Self::Ime) event for more complex character sequences.
    ReceivedCharacter(char),

    /// The window gained or lost focus.
    ///
    /// The parameter is true if the window has gained focus, and false if it has lost focus.
    Focused(bool),

    /// An event from the keyboard has been received.
    KeyboardInput {
        device_id: DeviceId,
        input: KeyboardInput,
        /// If `true`, the event was generated synthetically by winit
        /// in one of the following circumstances:
        ///
        /// * Synthetic key press events are generated for all keys pressed
        ///   when a window gains focus. Likewise, synthetic key release events
        ///   are generated for all keys pressed when a window goes out of focus.
        ///   ***Currently, this is only functional on X11 and Windows***
        ///
        /// Otherwise, this value is always `false`.
        is_synthetic: bool,
    },

    /// The keyboard modifiers have changed.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** This API is currently unimplemented on the web. This isn't by design - it's an
    ///   issue, and it should get fixed - but it's the current state of the API.
    ModifiersChanged(ModifiersState),

    /// An event from an input method.
    ///
    /// **Note:** You have to explicitly enable this event using [`Window::set_ime_allowed`].
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web / Orbital:** Unsupported.
    Ime(Ime),

    /// The cursor has moved on the window.
    CursorMoved {
        device_id: DeviceId,

        /// (x,y) coords in pixels relative to the top-left corner of the window. Because the range of this data is
        /// limited by the display area and it may have been transformed by the OS to implement effects such as cursor
        /// acceleration, it should not be used to implement non-cursor-like interactions such as 3D camera control.
        position: PhysicalPosition<f64>,
        #[deprecated = "Deprecated in favor of WindowEvent::ModifiersChanged"]
        modifiers: ModifiersState,
    },

    /// The cursor has entered the window.
    CursorEntered { device_id: DeviceId },

    /// The cursor has left the window.
    CursorLeft { device_id: DeviceId },

    /// A mouse wheel movement or touchpad scroll occurred.
    MouseWheel {
        device_id: DeviceId,
        delta: MouseScrollDelta,
        phase: TouchPhase,
        #[deprecated = "Deprecated in favor of WindowEvent::ModifiersChanged"]
        modifiers: ModifiersState,
    },

    /// An mouse button press has been received.
    MouseInput {
        device_id: DeviceId,
        state: ElementState,
        button: MouseButton,
        #[deprecated = "Deprecated in favor of WindowEvent::ModifiersChanged"]
        modifiers: ModifiersState,
    },

    /// Touchpad magnification event with two-finger pinch gesture.
    ///
    /// Positive delta values indicate magnification (zooming in) and
    /// negative delta values indicate shrinking (zooming out).
    ///
    /// ## Platform-specific
    ///
    /// - Only available on **macOS**.
    TouchpadMagnify {
        device_id: DeviceId,
        delta: f64,
        phase: TouchPhase,
    },

    /// Smart magnification event.
    ///
    /// On a Mac, smart magnification is triggered by a double tap with two fingers
    /// on the trackpad and is commonly used to zoom on a certain object
    /// (e.g. a paragraph of a PDF) or (sort of like a toggle) to reset any zoom.
    /// The gesture is also supported in Safari, Pages, etc.
    ///
    /// The event is general enough that its generating gesture is allowed to vary
    /// across platforms. It could also be generated by another device.
    ///
    /// Unfortunatly, neither [Windows](https://support.microsoft.com/en-us/windows/touch-gestures-for-windows-a9d28305-4818-a5df-4e2b-e5590f850741)
    /// nor [Wayland](https://wayland.freedesktop.org/libinput/doc/latest/gestures.html)
    /// support this gesture or any other gesture with the same effect.
    ///
    /// ## Platform-specific
    ///
    /// - Only available on **macOS 10.8** and later.
    SmartMagnify { device_id: DeviceId },

    /// Touchpad rotation event with two-finger rotation gesture.
    ///
    /// Positive delta values indicate rotation counterclockwise and
    /// negative delta values indicate rotation clockwise.
    ///
    /// ## Platform-specific
    ///
    /// - Only available on **macOS**.
    TouchpadRotate {
        device_id: DeviceId,
        delta: f32,
        phase: TouchPhase,
    },

    /// Touchpad pressure event.
    ///
    /// At the moment, only supported on Apple forcetouch-capable macbooks.
    /// The parameters are: pressure level (value between 0 and 1 representing how hard the touchpad
    /// is being pressed) and stage (integer representing the click level).
    TouchpadPressure {
        device_id: DeviceId,
        pressure: f32,
        stage: i64,
    },

    /// Motion on some analog axis. May report data redundant to other, more specific events.
    AxisMotion {
        device_id: DeviceId,
        axis: AxisId,
        value: f64,
    },

    /// Touch event has been received
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
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
    /// ## Platform-specific
    ///
    /// - **iOS / Android / X11 / Wayland / Orbital:** Unsupported.
    ThemeChanged(Theme),

    /// The window has been occluded (completely hidden from view).
    ///
    /// This is different to window visibility as it depends on whether the window is closed,
    /// minimised, set invisible, or fully occluded by another window.
    ///
    /// Platform-specific behavior:
    /// - **iOS / Android / Web / Wayland / Windows / Orbital:** Unsupported.
    Occluded(bool),
}

impl Clone for WindowEvent<'static> {
    fn clone(&self) -> Self {
        use self::WindowEvent::*;
        return match self {
            Resized(size) => Resized(*size),
            Moved(pos) => Moved(*pos),
            CloseRequested => CloseRequested,
            Destroyed => Destroyed,
            DroppedFile(file) => DroppedFile(file.clone()),
            HoveredFile(file) => HoveredFile(file.clone()),
            HoveredFileCancelled => HoveredFileCancelled,
            ReceivedCharacter(c) => ReceivedCharacter(*c),
            Focused(f) => Focused(*f),
            KeyboardInput {
                device_id,
                input,
                is_synthetic,
            } => KeyboardInput {
                device_id: *device_id,
                input: *input,
                is_synthetic: *is_synthetic,
            },
            Ime(preedit_state) => Ime(preedit_state.clone()),
            ModifiersChanged(modifiers) => ModifiersChanged(*modifiers),
            #[allow(deprecated)]
            CursorMoved {
                device_id,
                position,
                modifiers,
            } => CursorMoved {
                device_id: *device_id,
                position: *position,
                modifiers: *modifiers,
            },
            CursorEntered { device_id } => CursorEntered {
                device_id: *device_id,
            },
            CursorLeft { device_id } => CursorLeft {
                device_id: *device_id,
            },
            #[allow(deprecated)]
            MouseWheel {
                device_id,
                delta,
                phase,
                modifiers,
            } => MouseWheel {
                device_id: *device_id,
                delta: *delta,
                phase: *phase,
                modifiers: *modifiers,
            },
            #[allow(deprecated)]
            MouseInput {
                device_id,
                state,
                button,
                modifiers,
            } => MouseInput {
                device_id: *device_id,
                state: *state,
                button: *button,
                modifiers: *modifiers,
            },
            TouchpadMagnify {
                device_id,
                delta,
                phase,
            } => TouchpadMagnify {
                device_id: *device_id,
                delta: *delta,
                phase: *phase,
            },
            SmartMagnify { device_id } => SmartMagnify {
                device_id: *device_id,
            },
            TouchpadRotate {
                device_id,
                delta,
                phase,
            } => TouchpadRotate {
                device_id: *device_id,
                delta: *delta,
                phase: *phase,
            },
            TouchpadPressure {
                device_id,
                pressure,
                stage,
            } => TouchpadPressure {
                device_id: *device_id,
                pressure: *pressure,
                stage: *stage,
            },
            AxisMotion {
                device_id,
                axis,
                value,
            } => AxisMotion {
                device_id: *device_id,
                axis: *axis,
                value: *value,
            },
            Touch(touch) => Touch(*touch),
            ThemeChanged(theme) => ThemeChanged(*theme),
            ScaleFactorChanged { .. } => {
                unreachable!("Static event can't be about scale factor changing")
            }
            Occluded(occluded) => Occluded(*occluded),
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
            KeyboardInput {
                device_id,
                input,
                is_synthetic,
            } => Some(KeyboardInput {
                device_id,
                input,
                is_synthetic,
            }),
            ModifiersChanged(modifiers) => Some(ModifiersChanged(modifiers)),
            Ime(event) => Some(Ime(event)),
            #[allow(deprecated)]
            CursorMoved {
                device_id,
                position,
                modifiers,
            } => Some(CursorMoved {
                device_id,
                position,
                modifiers,
            }),
            CursorEntered { device_id } => Some(CursorEntered { device_id }),
            CursorLeft { device_id } => Some(CursorLeft { device_id }),
            #[allow(deprecated)]
            MouseWheel {
                device_id,
                delta,
                phase,
                modifiers,
            } => Some(MouseWheel {
                device_id,
                delta,
                phase,
                modifiers,
            }),
            #[allow(deprecated)]
            MouseInput {
                device_id,
                state,
                button,
                modifiers,
            } => Some(MouseInput {
                device_id,
                state,
                button,
                modifiers,
            }),
            TouchpadMagnify {
                device_id,
                delta,
                phase,
            } => Some(TouchpadMagnify {
                device_id,
                delta,
                phase,
            }),
            SmartMagnify { device_id } => Some(SmartMagnify { device_id }),
            TouchpadRotate {
                device_id,
                delta,
                phase,
            } => Some(TouchpadRotate {
                device_id,
                delta,
                phase,
            }),
            TouchpadPressure {
                device_id,
                pressure,
                stage,
            } => Some(TouchpadPressure {
                device_id,
                pressure,
                stage,
            }),
            AxisMotion {
                device_id,
                axis,
                value,
            } => Some(AxisMotion {
                device_id,
                axis,
                value,
            }),
            Touch(touch) => Some(Touch(touch)),
            ThemeChanged(theme) => Some(ThemeChanged(theme)),
            ScaleFactorChanged { .. } => None,
            Occluded(occluded) => Some(Occluded(occluded)),
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
    /// Returns a dummy id, useful for unit testing.
    ///
    /// # Safety
    ///
    /// The only guarantee made about the return value of this function is that
    /// it will always be equal to itself and to future values returned by this function.
    /// No other guarantees are made. This may be equal to a real `DeviceId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub const unsafe fn dummy() -> Self {
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
    /// This represents raw, unfiltered physical motion. Not to be confused with [`WindowEvent::CursorMoved`].
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

    /// Motion on some analog axis. This event will be reported for all arbitrary input devices
    /// that winit supports on this platform, including mouse devices.  If the device is a mouse
    /// device then this will be reported alongside the MouseMotion event.
    Motion {
        axis: AxisId,
        value: f64,
    },

    Button {
        button: ButtonId,
        state: ElementState,
    },

    Key(KeyboardInput),

    Text {
        codepoint: char,
    },
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
    #[deprecated = "Deprecated in favor of WindowEvent::ModifiersChanged"]
    pub modifiers: ModifiersState,
}

/// Describes [input method](https://en.wikipedia.org/wiki/Input_method) events.
///
/// This is also called a "composition event".
///
/// Most keypresses using a latin-like keyboard layout simply generate a [`WindowEvent::ReceivedCharacter`].
/// However, one couldn't possibly have a key for every single unicode character that the user might want to type
/// - so the solution operating systems employ is to allow the user to type these using _a sequence of keypresses_ instead.
///
/// A prominent example of this is accents - many keyboard layouts allow you to first click the "accent key", and then
/// the character you want to apply the accent to. This will generate the following event sequence:
/// ```ignore
/// // Press "`" key
/// Ime::Preedit("`", Some((0, 0)))
/// // Press "E" key
/// Ime::Preedit("", None) // Synthetic event generated by winit to clear preedit.
/// Ime::Commit("é")
/// ```
///
/// Additionally, certain input devices are configured to display a candidate box that allow the user to select the
/// desired character interactively. (To properly position this box, you must use [`Window::set_ime_position`].)
///
/// An example of a keyboard layout which uses candidate boxes is pinyin. On a latin keybaord the following event
/// sequence could be obtained:
/// ```ignore
/// // Press "A" key
/// Ime::Preedit("a", Some((1, 1)))
/// // Press "B" key
/// Ime::Preedit("a b", Some((3, 3)))
/// // Press left arrow key
/// Ime::Preedit("a b", Some((1, 1)))
/// // Press space key
/// Ime::Preedit("啊b", Some((3, 3)))
/// // Press space key
/// Ime::Preedit("", None) // Synthetic event generated by winit to clear preedit.
/// Ime::Commit("啊不")
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Ime {
    /// Notifies when the IME was enabled.
    ///
    /// After getting this event you could receive [`Preedit`](Self::Preedit) and
    /// [`Commit`](Self::Commit) events. You should also start performing IME related requests
    /// like [`Window::set_ime_position`].
    Enabled,

    /// Notifies when a new composing text should be set at the cursor position.
    ///
    /// The value represents a pair of the preedit string and the cursor begin position and end
    /// position. When it's `None`, the cursor should be hidden. When `String` is an empty string
    /// this indicates that preedit was cleared.
    ///
    /// The cursor position is byte-wise indexed.
    Preedit(String, Option<(usize, usize)>),

    /// Notifies when text should be inserted into the editor widget.
    ///
    /// Right before this event winit will send empty [`Self::Preedit`] event.
    Commit(String),

    /// Notifies when the IME was disabled.
    ///
    /// After receiving this event you won't get any more [`Preedit`](Self::Preedit) or
    /// [`Commit`](Self::Commit) events until the next [`Enabled`](Self::Enabled) event. You should
    /// also stop issuing IME related requests like [`Window::set_ime_position`] and clear pending
    /// preedit text.
    Disabled,
}

/// Describes touch-screen input state.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// Represents a touch event
///
/// Every time the user touches the screen, a new [`TouchPhase::Started`] event with an unique
/// identifier for the finger is generated. When the finger is lifted, an [`TouchPhase::Ended`]
/// event is generated with the same finger id.
///
/// After a `Started` event has been emitted, there may be zero or more `Move`
/// events when the finger is moved or the touch pressure changes.
///
/// The finger id may be reused by the system after an `Ended` event. The user
/// should assume that a new `Started` event received with the same id has nothing
/// to do with the old finger and is a new finger.
///
/// A [`TouchPhase::Cancelled`] event is emitted when the system has canceled tracking this
/// touch, such as when the window loses focus, or on iOS if the user moves the
/// device against their face.
///
/// ## Platform-specific
///
/// - **macOS:** Unsupported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    pub device_id: DeviceId,
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
    ///
    /// Instead of normalizing the force, you should prefer to handle
    /// [`Force::Calibrated`] so that the amount of force the user has to apply is
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
    Other(u16),
}

/// Describes a difference in the mouse scroll wheel state.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MouseScrollDelta {
    /// Amount in lines or rows to scroll in the horizontal
    /// and vertical directions.
    ///
    /// Positive values indicate that the content that is being scrolled should move
    /// right and down (revealing more content left and up).
    LineDelta(f32, f32),

    /// Amount in pixels to scroll in the horizontal and
    /// vertical direction.
    ///
    /// Scroll events are expressed as a `PixelDelta` if
    /// supported by the device (eg. a touchpad) and
    /// platform.
    ///
    /// Positive values indicate that the content being scrolled should
    /// move right/down.
    ///
    /// For a 'natural scrolling' touch pad (that acts like a touch screen)
    /// this means moving your fingers right and down should give positive values,
    /// and move the content right and down (to reveal more things left and up).
    PixelDelta(PhysicalPosition<f64>),
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
        const SHIFT = 0b100;
        // const LSHIFT = 0b010;
        // const RSHIFT = 0b001;
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
