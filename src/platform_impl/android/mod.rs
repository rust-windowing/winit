use std::cell::Cell;
use std::collections::VecDeque;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use android_activity::input::{InputEvent, KeyAction, Keycode, MotionAction};
use android_activity::{
    AndroidApp, AndroidAppWaker, ConfigurationRef, InputStatus, MainEvent, Rect,
};
use tracing::{debug, trace, warn};

use crate::cursor::Cursor;
use crate::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use crate::error;
use crate::error::EventLoopError;
use crate::event::{self, Force, InnerSizeWriter, StartCause};
use crate::event_loop::{self, ActiveEventLoop as RootAEL, ControlFlow, DeviceEvents};
use crate::platform::pump_events::PumpStatus;
use crate::platform_impl::Fullscreen;
use crate::window::{
    self, CursorGrabMode, CustomCursor, CustomCursorSource, ImePurpose, ResizeDirection, Theme,
    WindowButtons, WindowLevel,
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

struct PeekableReceiver<T> {
    recv: mpsc::Receiver<T>,
    first: Option<T>,
}

impl<T> PeekableReceiver<T> {
    pub fn from_recv(recv: mpsc::Receiver<T>) -> Self {
        Self { recv, first: None }
    }

    pub fn has_incoming(&mut self) -> bool {
        if self.first.is_some() {
            return true;
        }
        match self.recv.try_recv() {
            Ok(v) => {
                self.first = Some(v);
                true
            },
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                warn!("Channel was disconnected when checking incoming");
                false
            },
        }
    }

    pub fn try_recv(&mut self) -> Result<T, mpsc::TryRecvError> {
        if let Some(first) = self.first.take() {
            return Ok(first);
        }
        self.recv.try_recv()
    }
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

pub struct EventLoop<T: 'static> {
    pub(crate) android_app: AndroidApp,
    window_target: event_loop::ActiveEventLoop,
    redraw_flag: SharedFlag,
    user_events_sender: mpsc::Sender<T>,
    user_events_receiver: PeekableReceiver<T>, // must wake looper whenever something gets sent
    loop_running: bool,                        // Dispatched `NewEvents<Init>`
    running: bool,
    pending_redraw: bool,
    cause: StartCause,
    ignore_volume_keys: bool,
    combining_accent: Option<char>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlatformSpecificEventLoopAttributes {
    pub(crate) android_app: Option<AndroidApp>,
    pub(crate) ignore_volume_keys: bool,
}

impl Default for PlatformSpecificEventLoopAttributes {
    fn default() -> Self {
        Self { android_app: Default::default(), ignore_volume_keys: true }
    }
}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(
        attributes: &PlatformSpecificEventLoopAttributes,
    ) -> Result<Self, EventLoopError> {
        let (user_events_sender, user_events_receiver) = mpsc::channel();

        let android_app = attributes.android_app.as_ref().expect(
            "An `AndroidApp` as passed to android_main() is required to create an `EventLoop` on \
             Android",
        );
        let redraw_flag = SharedFlag::new();

        Ok(Self {
            android_app: android_app.clone(),
            window_target: event_loop::ActiveEventLoop {
                p: ActiveEventLoop {
                    app: android_app.clone(),
                    control_flow: Cell::new(ControlFlow::default()),
                    exit: Cell::new(false),
                    redraw_requester: RedrawRequester::new(
                        &redraw_flag,
                        android_app.create_waker(),
                    ),
                },
                _marker: PhantomData,
            },
            redraw_flag,
            user_events_sender,
            user_events_receiver: PeekableReceiver::from_recv(user_events_receiver),
            loop_running: false,
            running: false,
            pending_redraw: false,
            cause: StartCause::Init,
            ignore_volume_keys: attributes.ignore_volume_keys,
            combining_accent: None,
        })
    }

    fn single_iteration<F>(&mut self, main_event: Option<MainEvent<'_>>, callback: &mut F)
    where
        F: FnMut(event::Event<T>, &RootAEL),
    {
        trace!("Mainloop iteration");

        let cause = self.cause;
        let mut pending_redraw = self.pending_redraw;
        let mut resized = false;

        callback(event::Event::NewEvents(cause), self.window_target());

        if let Some(event) = main_event {
            trace!("Handling main event {:?}", event);

            match event {
                MainEvent::InitWindow { .. } => {
                    callback(event::Event::Resumed, self.window_target());
                },
                MainEvent::TerminateWindow { .. } => {
                    callback(event::Event::Suspended, self.window_target());
                },
                MainEvent::WindowResized { .. } => resized = true,
                MainEvent::RedrawNeeded { .. } => pending_redraw = true,
                MainEvent::ContentRectChanged { .. } => {
                    warn!("TODO: find a way to notify application of content rect change");
                },
                MainEvent::GainedFocus => {
                    HAS_FOCUS.store(true, Ordering::Relaxed);
                    callback(
                        event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::Focused(true),
                        },
                        self.window_target(),
                    );
                },
                MainEvent::LostFocus => {
                    HAS_FOCUS.store(false, Ordering::Relaxed);
                    callback(
                        event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::Focused(false),
                        },
                        self.window_target(),
                    );
                },
                MainEvent::ConfigChanged { .. } => {
                    let monitor = MonitorHandle::new(self.android_app.clone());
                    let old_scale_factor = monitor.scale_factor();
                    let scale_factor = monitor.scale_factor();
                    if (scale_factor - old_scale_factor).abs() < f64::EPSILON {
                        let new_inner_size = Arc::new(Mutex::new(
                            MonitorHandle::new(self.android_app.clone()).size(),
                        ));
                        let event = event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::ScaleFactorChanged {
                                inner_size_writer: InnerSizeWriter::new(Arc::downgrade(
                                    &new_inner_size,
                                )),
                                scale_factor,
                            },
                        };
                        callback(event, self.window_target());
                    }
                },
                MainEvent::LowMemory => {
                    callback(event::Event::MemoryWarning, self.window_target());
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
        match android_app.input_events_iter() {
            Ok(mut input_iter) => loop {
                let read_event =
                    input_iter.next(|event| self.handle_input_event(&android_app, event, callback));

                if !read_event {
                    break;
                }
            },
            Err(err) => {
                tracing::warn!("Failed to get input events iterator: {err:?}");
            },
        }

        // Empty the user event buffer
        {
            while let Ok(event) = self.user_events_receiver.try_recv() {
                callback(crate::event::Event::UserEvent(event), self.window_target());
            }
        }

        if self.running {
            if resized {
                let size = if let Some(native_window) = self.android_app.native_window().as_ref() {
                    let width = native_window.width() as _;
                    let height = native_window.height() as _;
                    PhysicalSize::new(width, height)
                } else {
                    PhysicalSize::new(0, 0)
                };
                let event = event::Event::WindowEvent {
                    window_id: window::WindowId(WindowId),
                    event: event::WindowEvent::Resized(size),
                };
                callback(event, self.window_target());
            }

            pending_redraw |= self.redraw_flag.get_and_reset();
            if pending_redraw {
                pending_redraw = false;
                let event = event::Event::WindowEvent {
                    window_id: window::WindowId(WindowId),
                    event: event::WindowEvent::RedrawRequested,
                };
                callback(event, self.window_target());
            }
        }

        // This is always the last event we dispatch before poll again
        callback(event::Event::AboutToWait, self.window_target());

        self.pending_redraw = pending_redraw;
    }

    fn handle_input_event<F>(
        &mut self,
        android_app: &AndroidApp,
        event: &InputEvent<'_>,
        callback: &mut F,
    ) -> InputStatus
    where
        F: FnMut(event::Event<T>, &RootAEL),
    {
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
                        let event = event::Event::WindowEvent {
                            window_id,
                            event: event::WindowEvent::Touch(event::Touch {
                                device_id,
                                phase,
                                location,
                                id: pointer.pointer_id() as u64,
                                force: Some(Force::Normalized(pointer.pressure() as f64)),
                            }),
                        };
                        callback(event, self.window_target());
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

                        let event = event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::KeyboardInput {
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
                            },
                        };
                        callback(event, self.window_target());
                    },
                }
            },
            _ => {
                warn!("Unknown android_activity input event {event:?}")
            },
        }

        input_status
    }

    pub fn run<F>(mut self, event_handler: F) -> Result<(), EventLoopError>
    where
        F: FnMut(event::Event<T>, &event_loop::ActiveEventLoop),
    {
        self.run_on_demand(event_handler)
    }

    pub fn run_on_demand<F>(&mut self, mut event_handler: F) -> Result<(), EventLoopError>
    where
        F: FnMut(event::Event<T>, &event_loop::ActiveEventLoop),
    {
        loop {
            match self.pump_events(None, &mut event_handler) {
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

    pub fn pump_events<F>(&mut self, timeout: Option<Duration>, mut callback: F) -> PumpStatus
    where
        F: FnMut(event::Event<T>, &RootAEL),
    {
        if !self.loop_running {
            self.loop_running = true;

            // Reset the internal state for the loop as we start running to
            // ensure consistent behaviour in case the loop runs and exits more
            // than once
            self.pending_redraw = false;
            self.cause = StartCause::Init;

            // run the initial loop iteration
            self.single_iteration(None, &mut callback);
        }

        // Consider the possibility that the `StartCause::Init` iteration could
        // request to Exit
        if !self.exiting() {
            self.poll_events_with_timeout(timeout, &mut callback);
        }
        if self.exiting() {
            self.loop_running = false;

            callback(event::Event::LoopExiting, self.window_target());

            PumpStatus::Exit(0)
        } else {
            PumpStatus::Continue
        }
    }

    fn poll_events_with_timeout<F>(&mut self, mut timeout: Option<Duration>, mut callback: F)
    where
        F: FnMut(event::Event<T>, &RootAEL),
    {
        let start = Instant::now();

        self.pending_redraw |= self.redraw_flag.get_and_reset();

        timeout =
            if self.running && (self.pending_redraw || self.user_events_receiver.has_incoming()) {
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

        let app = self.android_app.clone(); // Don't borrow self as part of poll expression
        app.poll_events(timeout, |poll_event| {
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
                        || (!self.pending_redraw && !self.user_events_receiver.has_incoming())
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

            self.single_iteration(main_event, &mut callback);
        });
    }

    pub fn window_target(&self) -> &event_loop::ActiveEventLoop {
        &self.window_target
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            user_events_sender: self.user_events_sender.clone(),
            waker: self.android_app.create_waker(),
        }
    }

    fn control_flow(&self) -> ControlFlow {
        self.window_target.p.control_flow()
    }

    fn exiting(&self) -> bool {
        self.window_target.p.exiting()
    }
}

pub struct EventLoopProxy<T: 'static> {
    user_events_sender: mpsc::Sender<T>,
    waker: AndroidAppWaker,
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy {
            user_events_sender: self.user_events_sender.clone(),
            waker: self.waker.clone(),
        }
    }
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), event_loop::EventLoopClosed<T>> {
        self.user_events_sender.send(event).map_err(|err| event_loop::EventLoopClosed(err.0))?;
        self.waker.wake();
        Ok(())
    }
}

pub struct ActiveEventLoop {
    pub(crate) app: AndroidApp,
    control_flow: Cell<ControlFlow>,
    exit: Cell<bool>,
    redraw_requester: RedrawRequester,
}

impl ActiveEventLoop {
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle::new(self.app.clone()))
    }

    pub fn create_custom_cursor(&self, source: CustomCursorSource) -> CustomCursor {
        let _ = source.inner;
        CustomCursor { inner: PlatformCustomCursor }
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle::new(self.app.clone()));
        v
    }

    #[inline]
    pub fn listen_device_events(&self, _allowed: DeviceEvents) {}

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::RawDisplayHandle::Android(rwh_05::AndroidDisplayHandle::empty())
    }

    #[inline]
    pub fn system_theme(&self) -> Option<Theme> {
        None
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Android(rwh_06::AndroidDisplayHandle::new()))
    }

    pub(crate) fn set_control_flow(&self, control_flow: ControlFlow) {
        self.control_flow.set(control_flow)
    }

    pub(crate) fn control_flow(&self) -> ControlFlow {
        self.control_flow.get()
    }

    pub(crate) fn exit(&self) {
        self.exit.set(true)
    }

    pub(crate) fn clear_exit(&self) {
        self.exit.set(false)
    }

    pub(crate) fn exiting(&self) -> bool {
        self.exit.get()
    }

    pub(crate) fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle
    }
}

#[derive(Clone)]
pub(crate) struct OwnedDisplayHandle;

impl OwnedDisplayHandle {
    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::AndroidDisplayHandle::empty().into()
    }

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

    pub(crate) fn maybe_queue_on_main(&self, f: impl FnOnce(&Self) + Send + 'static) {
        f(self)
    }

    pub(crate) fn maybe_wait_on_main<R: Send>(&self, f: impl FnOnce(&Self) -> R + Send) -> R {
        f(self)
    }

    pub fn id(&self) -> WindowId {
        WindowId
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle::new(self.app.clone()))
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle::new(self.app.clone()));
        v
    }

    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle::new(self.app.clone()))
    }

    pub fn scale_factor(&self) -> f64 {
        MonitorHandle::new(self.app.clone()).scale_factor()
    }

    pub fn request_redraw(&self) {
        self.redraw_requester.request_redraw()
    }

    pub fn pre_present_notify(&self) {}

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        Err(error::NotSupportedError::new())
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        Err(error::NotSupportedError::new())
    }

    pub fn set_outer_position(&self, _position: Position) {
        // no effect
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.outer_size()
    }

    pub fn request_inner_size(&self, _size: Size) -> Option<PhysicalSize<u32>> {
        Some(self.inner_size())
    }

    pub fn outer_size(&self) -> PhysicalSize<u32> {
        MonitorHandle::new(self.app.clone()).size()
    }

    pub fn set_min_inner_size(&self, _: Option<Size>) {}

    pub fn set_max_inner_size(&self, _: Option<Size>) {}

    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    pub fn set_resize_increments(&self, _increments: Option<Size>) {}

    pub fn set_title(&self, _title: &str) {}

    pub fn set_transparent(&self, _transparent: bool) {}

    pub fn set_blur(&self, _blur: bool) {}

    pub fn set_visible(&self, _visibility: bool) {}

    pub fn is_visible(&self) -> Option<bool> {
        None
    }

    pub fn set_resizable(&self, _resizeable: bool) {}

    pub fn is_resizable(&self) -> bool {
        false
    }

    pub fn set_enabled_buttons(&self, _buttons: WindowButtons) {}

    pub fn enabled_buttons(&self) -> WindowButtons {
        WindowButtons::all()
    }

    pub fn set_minimized(&self, _minimized: bool) {}

    pub fn is_minimized(&self) -> Option<bool> {
        None
    }

    pub fn set_maximized(&self, _maximized: bool) {}

    pub fn is_maximized(&self) -> bool {
        false
    }

    pub fn set_fullscreen(&self, _monitor: Option<Fullscreen>) {
        warn!("Cannot set fullscreen on Android");
    }

    pub fn fullscreen(&self) -> Option<Fullscreen> {
        None
    }

    pub fn set_decorations(&self, _decorations: bool) {}

    pub fn is_decorated(&self) -> bool {
        true
    }

    pub fn set_window_level(&self, _level: WindowLevel) {}

    pub fn set_window_icon(&self, _window_icon: Option<crate::icon::Icon>) {}

    pub fn set_ime_cursor_area(&self, _position: Position, _size: Size) {}

    pub fn set_ime_allowed(&self, allowed: bool) {
        if allowed {
            self.app.show_soft_input(true);
        } else {
            self.app.hide_soft_input(true);
        }
    }

    pub fn set_ime_purpose(&self, _purpose: ImePurpose) {}

    pub fn focus_window(&self) {}

    pub fn request_user_attention(&self, _request_type: Option<window::UserAttentionType>) {}

    pub fn set_cursor(&self, _: Cursor) {}

    pub fn set_cursor_position(&self, _: Position) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    pub fn set_cursor_grab(&self, _: CursorGrabMode) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    pub fn set_cursor_visible(&self, _: bool) {}

    pub fn drag_window(&self) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    pub fn drag_resize_window(
        &self,
        _direction: ResizeDirection,
    ) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    #[inline]
    pub fn show_window_menu(&self, _position: Position) {}

    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    #[cfg(feature = "rwh_04")]
    pub fn raw_window_handle_rwh_04(&self) -> rwh_04::RawWindowHandle {
        use rwh_04::HasRawWindowHandle;

        if let Some(native_window) = self.app.native_window().as_ref() {
            native_window.raw_window_handle()
        } else {
            panic!(
                "Cannot get the native window, it's null and will always be null before \
                 Event::Resumed and after Event::Suspended. Make sure you only call this function \
                 between those events."
            );
        }
    }

    #[cfg(feature = "rwh_05")]
    pub fn raw_window_handle_rwh_05(&self) -> rwh_05::RawWindowHandle {
        use rwh_05::HasRawWindowHandle;

        if let Some(native_window) = self.app.native_window().as_ref() {
            native_window.raw_window_handle()
        } else {
            panic!(
                "Cannot get the native window, it's null and will always be null before \
                 Event::Resumed and after Event::Suspended. Make sure you only call this function \
                 between those events."
            );
        }
    }

    #[cfg(feature = "rwh_05")]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::RawDisplayHandle::Android(rwh_05::AndroidDisplayHandle::empty())
    }

    #[cfg(feature = "rwh_06")]
    // Allow the usage of HasRawWindowHandle inside this function
    #[allow(deprecated)]
    pub fn raw_window_handle_rwh_06(&self) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
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
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Android(rwh_06::AndroidDisplayHandle::new()))
    }

    pub fn config(&self) -> ConfigurationRef {
        self.app.config()
    }

    pub fn content_rect(&self) -> Rect {
        self.app.content_rect()
    }

    pub fn set_theme(&self, _theme: Option<Theme>) {}

    pub fn theme(&self) -> Option<Theme> {
        None
    }

    pub fn set_content_protected(&self, _protected: bool) {}

    pub fn has_focus(&self) -> bool {
        HAS_FOCUS.load(Ordering::Relaxed)
    }

    pub fn title(&self) -> String {
        String::new()
    }

    pub fn reset_dead_keys(&self) {}
}

#[derive(Default, Clone, Debug)]
pub struct OsError;

use std::fmt::{self, Display, Formatter};
impl Display for OsError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(fmt, "Android OS Error")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MonitorHandle {
    app: AndroidApp,
}
impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for MonitorHandle {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}

impl MonitorHandle {
    pub(crate) fn new(app: AndroidApp) -> Self {
        Self { app }
    }

    pub fn name(&self) -> Option<String> {
        Some("Android Device".to_owned())
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        if let Some(native_window) = self.app.native_window() {
            PhysicalSize::new(native_window.width() as _, native_window.height() as _)
        } else {
            PhysicalSize::new(0, 0)
        }
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        (0, 0).into()
    }

    pub fn scale_factor(&self) -> f64 {
        self.app.config().density().map(|dpi| dpi as f64 / 160.0).unwrap_or(1.0)
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        // FIXME no way to get real refresh rate for now.
        None
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoModeHandle> {
        let size = self.size().into();
        // FIXME this is not the real refresh rate
        // (it is guaranteed to support 32 bit color though)
        std::iter::once(VideoModeHandle {
            size,
            bit_depth: 32,
            refresh_rate_millihertz: 60000,
            monitor: self.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoModeHandle {
    size: (u32, u32),
    bit_depth: u16,
    refresh_rate_millihertz: u32,
    monitor: MonitorHandle,
}

impl VideoModeHandle {
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size.into()
    }

    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.refresh_rate_millihertz
    }

    pub fn monitor(&self) -> MonitorHandle {
        self.monitor.clone()
    }
}
