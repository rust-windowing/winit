use std::cell::Cell;
use std::hash::Hash;
use std::mem::replace;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use android_activity::input::{InputEvent, KeyAction, Keycode, MotionAction};
use android_activity::{
    AndroidApp, AndroidAppWaker, ConfigurationRef, InputStatus, MainEvent, Rect,
};
use tracing::{debug, trace, warn};

use crate::application::ApplicationHandler;
use crate::cursor::Cursor;
use crate::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{self, EventLoopError, ExternalError, NotSupportedError};
use crate::event::{self, Force, InnerSizeWriter, StartCause};
use crate::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents,
    EventLoopProxy as RootEventLoopProxy, OwnedDisplayHandle as RootOwnedDisplayHandle,
};
use crate::monitor::MonitorHandle as RootMonitorHandle;
use crate::platform::pump_events::PumpStatus;
use crate::window::{
    self, CursorGrabMode, CustomCursor, CustomCursorSource, Fullscreen, ImePurpose,
    ResizeDirection, Theme, Window as CoreWindow, WindowAttributes, WindowButtons, WindowLevel,
};

mod keycodes;

pub(crate) use crate::cursor::{
    NoCustomCursor as PlatformCustomCursor, NoCustomCursor as PlatformCustomCursorSource,
};
pub(crate) use crate::icon::NoIcon as PlatformIcon;

static HAS_FOCUS: AtomicBool = AtomicBool::new(true);

/// Returns the minimum `Option<Duration>`, taking into account that `None`
/// equates to an infinite timeout, not a zero timeout (so can't just use
/// `Option::min`)
fn min_timeout(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    a.map_or(b, |a_timeout| b.map_or(Some(a_timeout), |b_timeout| Some(a_timeout.min(b_timeout))))
}

#[derive(Clone)]
struct SharedFlagSetter {
    flag: Arc<AtomicBool>,
}
impl SharedFlagSetter {
    pub fn set(&self) -> bool {
        self.flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed).is_ok()
    }
}

struct SharedFlag {
    flag: Arc<AtomicBool>,
}

// Used for queuing redraws from arbitrary threads. We don't care how many
// times a redraw is requested (so don't actually need to queue any data,
// we just need to know at the start of a main loop iteration if a redraw
// was queued and be able to read and clear the state atomically)
impl SharedFlag {
    pub fn new() -> Self {
        Self { flag: Arc::new(AtomicBool::new(false)) }
    }

    pub fn setter(&self) -> SharedFlagSetter {
        SharedFlagSetter { flag: self.flag.clone() }
    }

    pub fn get_and_reset(&self) -> bool {
        self.flag.swap(false, std::sync::atomic::Ordering::AcqRel)
    }
}

#[derive(Clone)]
pub struct RedrawRequester {
    flag: SharedFlagSetter,
    waker: AndroidAppWaker,
}

impl RedrawRequester {
    fn new(flag: &SharedFlag, waker: AndroidAppWaker) -> Self {
        RedrawRequester { flag: flag.setter(), waker }
    }

    pub fn request_redraw(&self) {
        if self.flag.set() {
            // Only explicitly try to wake up the main loop when the flag
            // value changes
            self.waker.wake();
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct KeyEventExtra {}

pub struct EventLoop {
    pub(crate) android_app: AndroidApp,
    window_target: ActiveEventLoop,
    redraw_flag: SharedFlag,
    loop_running: bool, // Dispatched `NewEvents<Init>`
    running: bool,
    pending_redraw: bool,
    cause: StartCause,
    ignore_volume_keys: bool,
    combining_accent: Option<char>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {
    pub(crate) android_app: Option<AndroidApp>,
    pub(crate) ignore_volume_keys: bool,
}

impl Default for PlatformSpecificEventLoopAttributes {
    fn default() -> Self {
        Self { android_app: Default::default(), ignore_volume_keys: true }
    }
}

enum ApplicationState<A, F> {
    SurfacesNotReady(F),
    Ready(A),
    Invalid,
}

impl<A: ApplicationHandler, F: FnOnce(&dyn RootActiveEventLoop) -> A> ApplicationState<A, F> {
    fn surfaces_ready(&mut self, event_loop: &dyn RootActiveEventLoop) {
        // Temporarily transition to an invalid state to make the compiler happy
        match replace(self, Self::Invalid) {
            Self::SurfacesNotReady(handler) => {
                let mut app = handler(event_loop);
                // We run lifecycle events here that didn't get emitted before now because the
                // application wasn't yet ready.
                app.new_events(event_loop, StartCause::Init);

                *self = Self::Ready(app);
            },
            Self::Ready(mut app) => {
                app.recreate_surfaces(event_loop);
                *self = Self::Ready(app);
            },
            Self::Invalid => unreachable!("invalid state"),
        }
    }

    #[track_caller]
    fn handle_if_ready(&mut self, closure: impl FnOnce(&mut A)) {
        match self {
            Self::SurfacesNotReady(_) => {
                tracing::trace!("event happened before application handler was initialized")
            },
            Self::Ready(app) => closure(app),
            Self::Invalid => unreachable!("invalid state"),
        }
    }

    #[track_caller]
    fn handle(&mut self, closure: impl FnOnce(&mut A)) {
        match self {
            Self::SurfacesNotReady(_) => {
                tracing::error!("tried to call application handler before it being initialized")
            },
            Self::Ready(app) => closure(app),
            Self::Invalid => unreachable!("invalid state"),
        }
    }
}

impl EventLoop {
    pub(crate) fn new(
        attributes: &PlatformSpecificEventLoopAttributes,
    ) -> Result<Self, EventLoopError> {
        let proxy_wake_up = Arc::new(AtomicBool::new(false));

        let android_app = attributes.android_app.as_ref().expect(
            "An `AndroidApp` as passed to android_main() is required to create an `EventLoop` on \
             Android",
        );
        let redraw_flag = SharedFlag::new();

        Ok(Self {
            android_app: android_app.clone(),
            window_target: ActiveEventLoop {
                app: android_app.clone(),
                control_flow: Cell::new(ControlFlow::default()),
                exit: Cell::new(false),
                redraw_requester: RedrawRequester::new(&redraw_flag, android_app.create_waker()),
                proxy_wake_up,
            },
            redraw_flag,
            loop_running: false,
            running: false,
            pending_redraw: false,
            cause: StartCause::Init,
            ignore_volume_keys: attributes.ignore_volume_keys,
            combining_accent: None,
        })
    }

    pub(crate) fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.window_target
    }

    fn single_iteration<A: ApplicationHandler, F: FnOnce(&dyn RootActiveEventLoop) -> A>(
        &mut self,
        main_event: Option<MainEvent<'_>>,
        app_state: &mut ApplicationState<A, F>,
    ) {
        trace!("Mainloop iteration");

        let cause = self.cause;
        let mut pending_redraw = self.pending_redraw;
        let mut resized = false;

        app_state.handle_if_ready(|app| app.new_events(&self.window_target, cause));

        if let Some(event) = main_event {
            trace!("Handling main event {:?}", event);

            match event {
                MainEvent::InitWindow { .. } => {
                    app_state.surfaces_ready(&self.window_target);
                },
                MainEvent::TerminateWindow { .. } => {
                    app_state.handle(|app| app.destroy_surfaces(&self.window_target));
                },
                MainEvent::WindowResized { .. } => resized = true,
                MainEvent::RedrawNeeded { .. } => pending_redraw = true,
                MainEvent::ContentRectChanged { .. } => {
                    warn!("TODO: find a way to notify application of content rect change");
                },
                MainEvent::GainedFocus => {
                    HAS_FOCUS.store(true, Ordering::Relaxed);
                    let window_id = window::WindowId(WindowId);
                    let event = event::WindowEvent::Focused(true);
                    app_state.handle(|app| app.window_event(&self.window_target, window_id, event));
                },
                MainEvent::LostFocus => {
                    HAS_FOCUS.store(false, Ordering::Relaxed);
                    let window_id = window::WindowId(WindowId);
                    let event = event::WindowEvent::Focused(false);
                    app_state.handle(|app| app.window_event(&self.window_target, window_id, event));
                },
                MainEvent::ConfigChanged { .. } => {
                    let old_scale_factor = scale_factor(&self.android_app);
                    let scale_factor = scale_factor(&self.android_app);
                    if (scale_factor - old_scale_factor).abs() < f64::EPSILON {
                        let new_inner_size = Arc::new(Mutex::new(screen_size(&self.android_app)));
                        let window_id = window::WindowId(WindowId);
                        let event = event::WindowEvent::ScaleFactorChanged {
                            inner_size_writer: InnerSizeWriter::new(Arc::downgrade(
                                &new_inner_size,
                            )),
                            scale_factor,
                        };

                        app_state
                            .handle(|app| app.window_event(&self.window_target, window_id, event));
                    }
                },
                MainEvent::LowMemory => {
                    app_state.handle(|app| app.memory_warning(&self.window_target));
                },
                MainEvent::Start => {
                    // XXX: how to forward this state to applications?
                    warn!("TODO: forward onStart notification to application");
                },
                MainEvent::Resume { .. } => {
                    debug!("App Resumed - is running");
                    self.running = true;
                },
                MainEvent::SaveState { .. } => {
                    // XXX: how to forward this state to applications?
                    // XXX: also how do we expose state restoration to apps?
                    warn!("TODO: forward saveState notification to application");
                },
                MainEvent::Pause => {
                    debug!("App Paused - stopped running");
                    self.running = false;
                },
                MainEvent::Stop => {
                    // XXX: how to forward this state to applications?
                    warn!("TODO: forward onStop notification to application");
                },
                MainEvent::Destroy => {
                    // XXX: maybe exit mainloop to drop things before being
                    // killed by the OS?
                    warn!("TODO: forward onDestroy notification to application");
                },
                MainEvent::InsetsChanged { .. } => {
                    // XXX: how to forward this state to applications?
                    warn!("TODO: handle Android InsetsChanged notification");
                },
                unknown => {
                    trace!("Unknown MainEvent {unknown:?} (ignored)");
                },
            }
        } else {
            trace!("No main event to handle");
        }

        // temporarily decouple `android_app` from `self` so we aren't holding
        // a borrow of `self` while iterating
        let android_app = self.android_app.clone();

        // Process input events
        app_state.handle_if_ready(|app| match android_app.input_events_iter() {
            Ok(mut input_iter) => loop {
                let read_event =
                    input_iter.next(|event| self.handle_input_event(&android_app, event, app));

                if !read_event {
                    break;
                }
            },
            Err(err) => {
                tracing::warn!("Failed to get input events iterator: {err:?}");
            },
        });

        app_state.handle_if_ready(|app| {
            if self.window_target.proxy_wake_up.swap(false, Ordering::Relaxed) {
                app.proxy_wake_up(&self.window_target);
            }
        });

        if self.running {
            if resized {
                let size = if let Some(native_window) = self.android_app.native_window().as_ref() {
                    let width = native_window.width() as _;
                    let height = native_window.height() as _;
                    PhysicalSize::new(width, height)
                } else {
                    PhysicalSize::new(0, 0)
                };
                let window_id = window::WindowId(WindowId);
                let event = event::WindowEvent::Resized(size);
                app_state.handle(|app| app.window_event(&self.window_target, window_id, event));
            }

            pending_redraw |= self.redraw_flag.get_and_reset();
            if pending_redraw {
                pending_redraw = false;
                let window_id = window::WindowId(WindowId);
                let event = event::WindowEvent::RedrawRequested;
                app_state.handle(|app| app.window_event(&self.window_target, window_id, event));
            }
        }

        // This is always the last event we dispatch before poll again
        app_state.handle_if_ready(|app| app.about_to_wait(&self.window_target));

        self.pending_redraw = pending_redraw;
    }

    fn handle_input_event<A: ApplicationHandler>(
        &mut self,
        android_app: &AndroidApp,
        event: &InputEvent<'_>,
        app: &mut A,
    ) -> InputStatus {
        let mut input_status = InputStatus::Handled;
        match event {
            InputEvent::MotionEvent(motion_event) => {
                let window_id = window::WindowId(WindowId);
                let device_id = event::DeviceId(DeviceId(motion_event.device_id()));

                let phase = match motion_event.action() {
                    MotionAction::Down | MotionAction::PointerDown => {
                        Some(event::TouchPhase::Started)
                    },
                    MotionAction::Up | MotionAction::PointerUp => Some(event::TouchPhase::Ended),
                    MotionAction::Move => Some(event::TouchPhase::Moved),
                    MotionAction::Cancel => Some(event::TouchPhase::Cancelled),
                    _ => {
                        None // TODO mouse events
                    },
                };
                if let Some(phase) = phase {
                    let pointers: Box<dyn Iterator<Item = android_activity::input::Pointer<'_>>> =
                        match phase {
                            event::TouchPhase::Started | event::TouchPhase::Ended => {
                                Box::new(std::iter::once(
                                    motion_event.pointer_at_index(motion_event.pointer_index()),
                                ))
                            },
                            event::TouchPhase::Moved | event::TouchPhase::Cancelled => {
                                Box::new(motion_event.pointers())
                            },
                        };

                    for pointer in pointers {
                        let location =
                            PhysicalPosition { x: pointer.x() as _, y: pointer.y() as _ };
                        trace!(
                            "Input event {device_id:?}, {phase:?}, loc={location:?}, \
                             pointer={pointer:?}"
                        );

                        let event = event::WindowEvent::Touch(event::Touch {
                            device_id,
                            phase,
                            location,
                            finger_id: event::FingerId(FingerId(pointer.pointer_id())),
                            force: Some(Force::Normalized(pointer.pressure() as f64)),
                        });

                        app.window_event(&self.window_target, window_id, event);
                    }
                }
            },
            InputEvent::KeyEvent(key) => {
                match key.key_code() {
                    // Flag keys related to volume as unhandled. While winit does not have a way for
                    // applications to configure what keys to flag as handled,
                    // this appears to be a good default until winit
                    // can be configured.
                    Keycode::VolumeUp | Keycode::VolumeDown | Keycode::VolumeMute
                        if self.ignore_volume_keys =>
                    {
                        input_status = InputStatus::Unhandled
                    },
                    keycode => {
                        let state = match key.action() {
                            KeyAction::Down => event::ElementState::Pressed,
                            KeyAction::Up => event::ElementState::Released,
                            _ => event::ElementState::Released,
                        };

                        let key_char = keycodes::character_map_and_combine_key(
                            android_app,
                            key,
                            &mut self.combining_accent,
                        );

                        let window_id = window::WindowId(WindowId);
                        let event = event::WindowEvent::KeyboardInput {
                            device_id: event::DeviceId(DeviceId(key.device_id())),
                            event: event::KeyEvent {
                                state,
                                physical_key: keycodes::to_physical_key(keycode),
                                logical_key: keycodes::to_logical(key_char, keycode),
                                location: keycodes::to_location(keycode),
                                repeat: key.repeat_count() > 0,
                                text: None,
                                platform_specific: KeyEventExtra {},
                            },
                            is_synthetic: false,
                        };

                        app.window_event(&self.window_target, window_id, event);
                    },
                }
            },
            _ => {
                warn!("Unknown android_activity input event {event:?}")
            },
        }

        input_status
    }

    pub fn run<A: ApplicationHandler>(
        mut self,
        init_closure: impl FnOnce(&dyn RootActiveEventLoop) -> A,
    ) -> Result<(), EventLoopError> {
        self.run_on_demand(init_closure)
    }

    pub fn run_on_demand<A: ApplicationHandler>(
        &mut self,
        init_closure: impl FnOnce(&dyn RootActiveEventLoop) -> A,
    ) -> Result<(), EventLoopError> {
        self.window_target.clear_exit();
        let mut app_state = ApplicationState::SurfacesNotReady(init_closure);
        loop {
            match self.pump_app_events_inner(None, &mut app_state) {
                PumpStatus::Exit(0) => {
                    break Ok(());
                },
                PumpStatus::Exit(code) => {
                    break Err(EventLoopError::ExitFailure(code));
                },
                _ => {
                    continue;
                },
            }
        }
    }

    pub fn pump_app_events<A: ApplicationHandler>(
        &mut self,
        timeout: Option<Duration>,
        app: A,
    ) -> PumpStatus {
        fn type_helper<A>(_event_loop: &dyn RootActiveEventLoop) -> A {
            unimplemented!()
        }
        #[allow(unused_assignments)]
        let mut app_state = ApplicationState::SurfacesNotReady(type_helper::<A>);
        app_state = ApplicationState::Ready(app);
        self.pump_app_events_inner(timeout, &mut app_state)
    }

    fn pump_app_events_inner<A: ApplicationHandler, F: FnOnce(&dyn RootActiveEventLoop) -> A>(
        &mut self,
        timeout: Option<Duration>,
        app_state: &mut ApplicationState<A, F>,
    ) -> PumpStatus {
        if !self.loop_running {
            self.loop_running = true;

            // Reset the internal state for the loop as we start running to
            // ensure consistent behaviour in case the loop runs and exits more
            // than once
            self.pending_redraw = false;
            self.cause = StartCause::Init;

            // run the initial loop iteration
            self.single_iteration(None, app_state);
        }

        // Consider the possibility that the `StartCause::Init` iteration could
        // request to Exit
        if !self.exiting() {
            self.poll_events_with_timeout(timeout, app_state);
        }
        if self.exiting() {
            self.loop_running = false;

            PumpStatus::Exit(0)
        } else {
            PumpStatus::Continue
        }
    }

    fn poll_events_with_timeout<A: ApplicationHandler, F: FnOnce(&dyn RootActiveEventLoop) -> A>(
        &mut self,
        mut timeout: Option<Duration>,
        app_state: &mut ApplicationState<A, F>,
    ) {
        let start = Instant::now();

        self.pending_redraw |= self.redraw_flag.get_and_reset();

        timeout = if self.running
            && (self.pending_redraw || self.window_target.proxy_wake_up.load(Ordering::Relaxed))
        {
            // If we already have work to do then we don't want to block on the next poll
            Some(Duration::ZERO)
        } else {
            let control_flow_timeout = match self.control_flow() {
                ControlFlow::Wait => None,
                ControlFlow::Poll => Some(Duration::ZERO),
                ControlFlow::WaitUntil(wait_deadline) => {
                    Some(wait_deadline.saturating_duration_since(start))
                },
            };

            min_timeout(control_flow_timeout, timeout)
        };

        let android_app = self.android_app.clone(); // Don't borrow self as part of poll expression
        android_app.poll_events(timeout, |poll_event| {
            let mut main_event = None;

            match poll_event {
                android_activity::PollEvent::Wake => {
                    // In the X11 backend it's noted that too many false-positive wake ups
                    // would cause the event loop to run continuously. They handle this by
                    // re-checking for pending events (assuming they cover all
                    // valid reasons for a wake up).
                    //
                    // For now, user_events and redraw_requests are the only reasons to expect
                    // a wake up here so we can ignore the wake up if there are no events/requests.
                    // We also ignore wake ups while suspended.
                    self.pending_redraw |= self.redraw_flag.get_and_reset();
                    if !self.running
                        || (!self.pending_redraw
                            && !self.window_target.proxy_wake_up.load(Ordering::Relaxed))
                    {
                        return;
                    }
                },
                android_activity::PollEvent::Timeout => {},
                android_activity::PollEvent::Main(event) => {
                    main_event = Some(event);
                },
                unknown_event => {
                    warn!("Unknown poll event {unknown_event:?} (ignored)");
                },
            }

            self.cause = match self.control_flow() {
                ControlFlow::Poll => StartCause::Poll,
                ControlFlow::Wait => StartCause::WaitCancelled { start, requested_resume: None },
                ControlFlow::WaitUntil(deadline) => {
                    if Instant::now() < deadline {
                        StartCause::WaitCancelled { start, requested_resume: Some(deadline) }
                    } else {
                        StartCause::ResumeTimeReached { start, requested_resume: deadline }
                    }
                },
            };

            self.single_iteration(main_event, app_state);
        });
    }

    fn control_flow(&self) -> ControlFlow {
        self.window_target.control_flow()
    }

    fn exiting(&self) -> bool {
        self.window_target.exiting()
    }
}

#[derive(Clone)]
pub struct EventLoopProxy {
    proxy_wake_up: Arc<AtomicBool>,
    waker: AndroidAppWaker,
}

impl EventLoopProxy {
    pub fn wake_up(&self) {
        self.proxy_wake_up.store(true, Ordering::Relaxed);
        self.waker.wake();
    }
}

pub struct ActiveEventLoop {
    pub(crate) app: AndroidApp,
    control_flow: Cell<ControlFlow>,
    exit: Cell<bool>,
    redraw_requester: RedrawRequester,
    proxy_wake_up: Arc<AtomicBool>,
}

impl ActiveEventLoop {
    fn clear_exit(&self) {
        self.exit.set(false);
    }
}

impl RootActiveEventLoop for ActiveEventLoop {
    fn create_proxy(&self) -> RootEventLoopProxy {
        let event_loop_proxy = EventLoopProxy {
            proxy_wake_up: self.proxy_wake_up.clone(),
            waker: self.app.create_waker(),
        };
        RootEventLoopProxy { event_loop_proxy }
    }

    fn create_window(
        &self,
        window_attributes: WindowAttributes,
    ) -> Result<Box<dyn CoreWindow>, error::OsError> {
        Ok(Box::new(Window::new(self, window_attributes)?))
    }

    fn create_custom_cursor(
        &self,
        _source: CustomCursorSource,
    ) -> Result<CustomCursor, ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = RootMonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        None
    }

    fn system_theme(&self) -> Option<Theme> {
        None
    }

    fn listen_device_events(&self, _allowed: DeviceEvents) {}

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.control_flow.set(control_flow)
    }

    fn control_flow(&self) -> ControlFlow {
        self.control_flow.get()
    }

    fn exit(&self) {
        self.exit.set(true)
    }

    fn exiting(&self) -> bool {
        self.exit.get()
    }

    fn owned_display_handle(&self) -> RootOwnedDisplayHandle {
        RootOwnedDisplayHandle { platform: OwnedDisplayHandle }
    }

    #[cfg(feature = "rwh_06")]
    fn rwh_06_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }
}

#[cfg(feature = "rwh_06")]
impl rwh_06::HasDisplayHandle for ActiveEventLoop {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::AndroidDisplayHandle::new();
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw.into()) })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct OwnedDisplayHandle;

impl OwnedDisplayHandle {
    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::AndroidDisplayHandle::new().into())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct WindowId;

impl WindowId {
    pub const fn dummy() -> Self {
        WindowId
    }
}

impl From<WindowId> for u64 {
    fn from(_: WindowId) -> Self {
        0
    }
}

impl From<u64> for WindowId {
    fn from(_: u64) -> Self {
        Self
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceId(i32);

impl DeviceId {
    pub const fn dummy() -> Self {
        DeviceId(0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FingerId(i32);

impl FingerId {
    pub const fn dummy() -> Self {
        FingerId(0)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformSpecificWindowAttributes;

pub(crate) struct Window {
    app: AndroidApp,
    redraw_requester: RedrawRequester,
}

impl Window {
    pub(crate) fn new(
        el: &ActiveEventLoop,
        _window_attrs: window::WindowAttributes,
    ) -> Result<Self, error::OsError> {
        // FIXME this ignores requested window attributes

        Ok(Self { app: el.app.clone(), redraw_requester: el.redraw_requester.clone() })
    }

    pub fn config(&self) -> ConfigurationRef {
        self.app.config()
    }

    pub fn content_rect(&self) -> Rect {
        self.app.content_rect()
    }

    #[cfg(feature = "rwh_06")]
    // Allow the usage of HasRawWindowHandle inside this function
    #[allow(deprecated)]
    fn raw_window_handle_rwh_06(&self) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
        use rwh_06::HasRawWindowHandle;

        if let Some(native_window) = self.app.native_window().as_ref() {
            native_window.raw_window_handle()
        } else {
            tracing::error!(
                "Cannot get the native window, it's null and will always be null before \
                 Event::Resumed and after Event::Suspended. Make sure you only call this function \
                 between those events."
            );
            Err(rwh_06::HandleError::Unavailable)
        }
    }

    #[cfg(feature = "rwh_06")]
    fn raw_display_handle_rwh_06(&self) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Android(rwh_06::AndroidDisplayHandle::new()))
    }
}

#[cfg(feature = "rwh_06")]
impl rwh_06::HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_display_handle_rwh_06()?;
        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw)) }
    }
}

#[cfg(feature = "rwh_06")]
impl rwh_06::HasWindowHandle for Window {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_window_handle_rwh_06()?;
        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw)) }
    }
}

impl CoreWindow for Window {
    fn id(&self) -> window::WindowId {
        window::WindowId(WindowId)
    }

    fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        None
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = RootMonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn current_monitor(&self) -> Option<RootMonitorHandle> {
        None
    }

    fn scale_factor(&self) -> f64 {
        scale_factor(&self.app)
    }

    fn request_redraw(&self) {
        self.redraw_requester.request_redraw()
    }

    fn pre_present_notify(&self) {}

    fn inner_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        Err(error::NotSupportedError::new())
    }

    fn outer_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        Err(error::NotSupportedError::new())
    }

    fn set_outer_position(&self, _position: Position) {
        // no effect
    }

    fn inner_size(&self) -> PhysicalSize<u32> {
        self.outer_size()
    }

    fn request_inner_size(&self, _size: Size) -> Option<PhysicalSize<u32>> {
        Some(self.inner_size())
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        screen_size(&self.app)
    }

    fn set_min_inner_size(&self, _: Option<Size>) {}

    fn set_max_inner_size(&self, _: Option<Size>) {}

    fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    fn set_resize_increments(&self, _increments: Option<Size>) {}

    fn set_title(&self, _title: &str) {}

    fn set_transparent(&self, _transparent: bool) {}

    fn set_blur(&self, _blur: bool) {}

    fn set_visible(&self, _visibility: bool) {}

    fn is_visible(&self) -> Option<bool> {
        None
    }

    fn set_resizable(&self, _resizeable: bool) {}

    fn is_resizable(&self) -> bool {
        false
    }

    fn set_enabled_buttons(&self, _buttons: WindowButtons) {}

    fn enabled_buttons(&self) -> WindowButtons {
        WindowButtons::all()
    }

    fn set_minimized(&self, _minimized: bool) {}

    fn is_minimized(&self) -> Option<bool> {
        None
    }

    fn set_maximized(&self, _maximized: bool) {}

    fn is_maximized(&self) -> bool {
        false
    }

    fn set_fullscreen(&self, _monitor: Option<Fullscreen>) {
        warn!("Cannot set fullscreen on Android");
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        None
    }

    fn set_decorations(&self, _decorations: bool) {}

    fn is_decorated(&self) -> bool {
        true
    }

    fn set_window_level(&self, _level: WindowLevel) {}

    fn set_window_icon(&self, _window_icon: Option<crate::icon::Icon>) {}

    fn set_ime_cursor_area(&self, _position: Position, _size: Size) {}

    fn set_ime_allowed(&self, _allowed: bool) {}

    fn set_ime_purpose(&self, _purpose: ImePurpose) {}

    fn focus_window(&self) {}

    fn request_user_attention(&self, _request_type: Option<window::UserAttentionType>) {}

    fn set_cursor(&self, _: Cursor) {}

    fn set_cursor_position(&self, _: Position) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    fn set_cursor_grab(&self, _: CursorGrabMode) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    fn set_cursor_visible(&self, _: bool) {}

    fn drag_window(&self) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    #[inline]
    fn show_window_menu(&self, _position: Position) {}

    fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    fn set_theme(&self, _theme: Option<Theme>) {}

    fn theme(&self) -> Option<Theme> {
        None
    }

    fn set_content_protected(&self, _protected: bool) {}

    fn has_focus(&self) -> bool {
        HAS_FOCUS.load(Ordering::Relaxed)
    }

    fn title(&self) -> String {
        String::new()
    }

    fn reset_dead_keys(&self) {}

    #[cfg(feature = "rwh_06")]
    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }

    #[cfg(feature = "rwh_06")]
    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        self
    }
}

#[derive(Default, Clone, Debug)]
pub struct OsError;

use std::fmt::{self, Display, Formatter};
impl Display for OsError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(fmt, "Android OS Error")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonitorHandle;

impl MonitorHandle {
    pub fn name(&self) -> Option<String> {
        unreachable!()
    }

    pub fn position(&self) -> Option<PhysicalPosition<i32>> {
        unreachable!()
    }

    pub fn scale_factor(&self) -> f64 {
        unreachable!()
    }

    pub fn current_video_mode(&self) -> Option<VideoModeHandle> {
        unreachable!()
    }

    pub fn video_modes(&self) -> std::iter::Empty<VideoModeHandle> {
        unreachable!()
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoModeHandle;

impl VideoModeHandle {
    pub fn size(&self) -> PhysicalSize<u32> {
        unreachable!()
    }

    pub fn bit_depth(&self) -> Option<NonZeroU16> {
        unreachable!()
    }

    pub fn refresh_rate_millihertz(&self) -> Option<NonZeroU32> {
        unreachable!()
    }

    pub fn monitor(&self) -> MonitorHandle {
        unreachable!()
    }
}

fn screen_size(app: &AndroidApp) -> PhysicalSize<u32> {
    if let Some(native_window) = app.native_window() {
        PhysicalSize::new(native_window.width() as _, native_window.height() as _)
    } else {
        PhysicalSize::new(0, 0)
    }
}

fn scale_factor(app: &AndroidApp) -> f64 {
    app.config().density().map(|dpi| dpi as f64 / 160.0).unwrap_or(1.0)
}
