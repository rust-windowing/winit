#![cfg(android_platform)]

use std::{
    collections::VecDeque,
    convert::TryInto,
    hash::Hash,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, RwLock,
    },
    time::{Duration, Instant},
};

use android_activity::input::{InputEvent, KeyAction, MotionAction};
use android_activity::{
    AndroidApp, AndroidAppWaker, ConfigurationRef, InputStatus, MainEvent, Rect,
};
use once_cell::sync::Lazy;
use raw_window_handle::{
    AndroidDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

#[cfg(feature = "android-native-activity")]
use ndk_sys::AKeyEvent_getKeyCode;

use crate::platform_impl::Fullscreen;
use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error,
    event::{self, StartCause},
    event_loop::{self, ControlFlow, EventLoopWindowTarget as RootELW},
    keyboard::{Key, KeyCode, KeyLocation, NativeKey, NativeKeyCode},
    window::{
        self, CursorGrabMode, ImePurpose, ResizeDirection, Theme, WindowButtons, WindowLevel,
    },
};

static HAS_FOCUS: Lazy<RwLock<bool>> = Lazy::new(|| RwLock::new(true));

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
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                warn!("Channel was disconnected when checking incoming");
                false
            }
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
        self.flag
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
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
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn setter(&self) -> SharedFlagSetter {
        SharedFlagSetter {
            flag: self.flag.clone(),
        }
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
        RedrawRequester {
            flag: flag.setter(),
            waker,
        }
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
    android_app: AndroidApp,
    window_target: event_loop::EventLoopWindowTarget<T>,
    redraw_flag: SharedFlag,
    user_events_sender: mpsc::Sender<T>,
    user_events_receiver: PeekableReceiver<T>, //must wake looper whenever something gets sent
    running: bool,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) struct PlatformSpecificEventLoopAttributes {
    pub(crate) android_app: Option<AndroidApp>,
}

fn sticky_exit_callback<T, F>(
    evt: event::Event<'_, T>,
    target: &RootELW<T>,
    control_flow: &mut ControlFlow,
    callback: &mut F,
) where
    F: FnMut(event::Event<'_, T>, &RootELW<T>, &mut ControlFlow),
{
    // make ControlFlow::ExitWithCode sticky by providing a dummy
    // control flow reference if it is already ExitWithCode.
    if let ControlFlow::ExitWithCode(code) = *control_flow {
        callback(evt, target, &mut ControlFlow::ExitWithCode(code))
    } else {
        callback(evt, target, control_flow)
    }
}

struct IterationResult {
    deadline: Option<Instant>,
    timeout: Option<Duration>,
    wait_start: Instant,
}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(attributes: &PlatformSpecificEventLoopAttributes) -> Self {
        let (user_events_sender, user_events_receiver) = mpsc::channel();

        let android_app = attributes.android_app.as_ref().expect("An `AndroidApp` as passed to android_main() is required to create an `EventLoop` on Android");
        let redraw_flag = SharedFlag::new();

        Self {
            android_app: android_app.clone(),
            window_target: event_loop::EventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    app: android_app.clone(),
                    redraw_requester: RedrawRequester::new(
                        &redraw_flag,
                        android_app.create_waker(),
                    ),
                    _marker: std::marker::PhantomData,
                },
                _marker: std::marker::PhantomData,
            },
            redraw_flag,
            user_events_sender,
            user_events_receiver: PeekableReceiver::from_recv(user_events_receiver),
            running: false,
        }
    }

    fn single_iteration<F>(
        &mut self,
        control_flow: &mut ControlFlow,
        main_event: Option<MainEvent<'_>>,
        pending_redraw: &mut bool,
        cause: &mut StartCause,
        callback: &mut F,
    ) -> IterationResult
    where
        F: FnMut(event::Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        trace!("Mainloop iteration");

        sticky_exit_callback(
            event::Event::NewEvents(*cause),
            self.window_target(),
            control_flow,
            callback,
        );

        let mut resized = false;

        if let Some(event) = main_event {
            trace!("Handling main event {:?}", event);

            match event {
                MainEvent::InitWindow { .. } => {
                    sticky_exit_callback(
                        event::Event::Resumed,
                        self.window_target(),
                        control_flow,
                        callback,
                    );
                }
                MainEvent::TerminateWindow { .. } => {
                    sticky_exit_callback(
                        event::Event::Suspended,
                        self.window_target(),
                        control_flow,
                        callback,
                    );
                }
                MainEvent::WindowResized { .. } => resized = true,
                MainEvent::RedrawNeeded { .. } => *pending_redraw = true,
                MainEvent::ContentRectChanged { .. } => {
                    warn!("TODO: find a way to notify application of content rect change");
                }
                MainEvent::GainedFocus => {
                    *HAS_FOCUS.write().unwrap() = true;
                    sticky_exit_callback(
                        event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::Focused(true),
                        },
                        self.window_target(),
                        control_flow,
                        callback,
                    );
                }
                MainEvent::LostFocus => {
                    *HAS_FOCUS.write().unwrap() = false;
                    sticky_exit_callback(
                        event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::Focused(false),
                        },
                        self.window_target(),
                        control_flow,
                        callback,
                    );
                }
                MainEvent::ConfigChanged { .. } => {
                    let monitor = MonitorHandle::new(self.android_app.clone());
                    let old_scale_factor = monitor.scale_factor();
                    let scale_factor = monitor.scale_factor();
                    if (scale_factor - old_scale_factor).abs() < f64::EPSILON {
                        let mut size = MonitorHandle::new(self.android_app.clone()).size();
                        let event = event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::ScaleFactorChanged {
                                new_inner_size: &mut size,
                                scale_factor,
                            },
                        };
                        sticky_exit_callback(event, self.window_target(), control_flow, callback);
                    }
                }
                MainEvent::LowMemory => {
                    // XXX: how to forward this state to applications?
                    // It seems like ideally winit should support lifecycle and
                    // low-memory events, especially for mobile platforms.
                    warn!("TODO: handle Android LowMemory notification");
                }
                MainEvent::Start => {
                    // XXX: how to forward this state to applications?
                    warn!("TODO: forward onStart notification to application");
                }
                MainEvent::Resume { .. } => {
                    debug!("App Resumed - is running");
                    self.running = true;
                }
                MainEvent::SaveState { .. } => {
                    // XXX: how to forward this state to applications?
                    // XXX: also how do we expose state restoration to apps?
                    warn!("TODO: forward saveState notification to application");
                }
                MainEvent::Pause => {
                    debug!("App Paused - stopped running");
                    self.running = false;
                }
                MainEvent::Stop => {
                    // XXX: how to forward this state to applications?
                    warn!("TODO: forward onStop notification to application");
                }
                MainEvent::Destroy => {
                    // XXX: maybe exit mainloop to drop things before being
                    // killed by the OS?
                    warn!("TODO: forward onDestroy notification to application");
                }
                MainEvent::InsetsChanged { .. } => {
                    // XXX: how to forward this state to applications?
                    warn!("TODO: handle Android InsetsChanged notification");
                }
                unknown => {
                    trace!("Unknown MainEvent {unknown:?} (ignored)");
                }
            }
        } else {
            trace!("No main event to handle");
        }

        // Process input events

        self.android_app.input_events(|event| {
            match event {
                InputEvent::MotionEvent(motion_event) => {
                    let window_id = window::WindowId(WindowId);
                    let device_id = event::DeviceId(DeviceId);

                    let phase = match motion_event.action() {
                        MotionAction::Down | MotionAction::PointerDown => {
                            Some(event::TouchPhase::Started)
                        }
                        MotionAction::Up | MotionAction::PointerUp => {
                            Some(event::TouchPhase::Ended)
                        }
                        MotionAction::Move => Some(event::TouchPhase::Moved),
                        MotionAction::Cancel => {
                            Some(event::TouchPhase::Cancelled)
                        }
                        _ => {
                            None // TODO mouse events
                        }
                    };
                    if let Some(phase) = phase {
                        let pointers: Box<
                            dyn Iterator<Item = android_activity::input::Pointer<'_>>,
                        > = match phase {
                            event::TouchPhase::Started
                            | event::TouchPhase::Ended => {
                                Box::new(
                                    std::iter::once(motion_event.pointer_at_index(
                                        motion_event.pointer_index(),
                                    ))
                                )
                            },
                            event::TouchPhase::Moved
                            | event::TouchPhase::Cancelled => {
                                Box::new(motion_event.pointers())
                            }
                        };

                        for pointer in pointers {
                            let location = PhysicalPosition {
                                x: pointer.x() as _,
                                y: pointer.y() as _,
                            };
                            trace!("Input event {device_id:?}, {phase:?}, loc={location:?}, pointer={pointer:?}");
                            let event = event::Event::WindowEvent {
                                window_id,
                                event: event::WindowEvent::Touch(
                                    event::Touch {
                                        device_id,
                                        phase,
                                        location,
                                        id: pointer.pointer_id() as u64,
                                        force: None,
                                    },
                                ),
                            };
                            sticky_exit_callback(
                                event,
                                self.window_target(),
                                control_flow,
                                callback
                            );
                        }
                    }
                }
                InputEvent::KeyEvent(key) => {
                    let state = match key.action() {
                        KeyAction::Down => event::ElementState::Pressed,
                        KeyAction::Up => event::ElementState::Released,
                        _ => event::ElementState::Released,
                    };

                    #[cfg(feature = "android-native-activity")]
                    let (keycode_u32, scancode_u32) = unsafe {
                        // We abuse the fact that `android_activity`'s `KeyEvent` is `repr(transparent)`
                        let event = (key as *const android_activity::input::KeyEvent<'_>).cast::<ndk::event::KeyEvent>();
                        // We use the unsafe function directly because we want to forward the
                        // keycode value even if it doesn't have a variant defined in the ndk
                        // crate.
                        (
                            AKeyEvent_getKeyCode((*event).ptr().as_ptr()) as u32,
                            (*event).scan_code() as u32
                        )
                    };
                    #[cfg(feature = "android-game-activity")]
                    let (keycode_u32, scancode_u32) = (key.keyCode as u32, key.scanCode as u32);
                    let keycode = keycode_u32
                        .try_into()
                        .unwrap_or(ndk::event::Keycode::Unknown);
                    let physical_key = KeyCode::Unidentified(
                        NativeKeyCode::Android(scancode_u32),
                    );
                    let native = NativeKey::Android(keycode_u32);
                    let logical_key = keycode_to_logical(keycode, native);
                    // TODO: maybe use getUnicodeChar to get the logical key

                    let event = event::Event::WindowEvent {
                        window_id: window::WindowId(WindowId),
                        event: event::WindowEvent::KeyboardInput {
                            device_id: event::DeviceId(DeviceId),
                            event: event::KeyEvent {
                                state,
                                physical_key,
                                logical_key,
                                location: keycode_to_location(keycode),
                                repeat: key.repeat_count() > 0,
                                text: None,
                                platform_specific: KeyEventExtra {},
                            },
                            is_synthetic: false,
                        },
                    };
                    sticky_exit_callback(
                        event,
                        self.window_target(),
                        control_flow,
                        callback,
                    );
                }
                _ => {
                    warn!("Unknown android_activity input event {event:?}")
                }
            }

            // Assume all events are handled, while Winit doesn't currently give a way for
            // applications to report whether they handled an input event.
            InputStatus::Handled
        });

        // Empty the user event buffer
        {
            while let Ok(event) = self.user_events_receiver.try_recv() {
                sticky_exit_callback(
                    crate::event::Event::UserEvent(event),
                    self.window_target(),
                    control_flow,
                    callback,
                );
            }
        }

        sticky_exit_callback(
            event::Event::MainEventsCleared,
            self.window_target(),
            control_flow,
            callback,
        );

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
                sticky_exit_callback(event, self.window_target(), control_flow, callback);
            }

            *pending_redraw |= self.redraw_flag.get_and_reset();
            if *pending_redraw {
                *pending_redraw = false;
                let event = event::Event::RedrawRequested(window::WindowId(WindowId));
                sticky_exit_callback(event, self.window_target(), control_flow, callback);
            }
        }

        sticky_exit_callback(
            event::Event::RedrawEventsCleared,
            self.window_target(),
            control_flow,
            callback,
        );

        let start = Instant::now();
        let (deadline, timeout);

        match control_flow {
            ControlFlow::ExitWithCode(_) => {
                deadline = None;
                timeout = None;
            }
            ControlFlow::Poll => {
                *cause = StartCause::Poll;
                deadline = None;
                timeout = Some(Duration::from_millis(0));
            }
            ControlFlow::Wait => {
                *cause = StartCause::WaitCancelled {
                    start,
                    requested_resume: None,
                };
                deadline = None;
                timeout = None;
            }
            ControlFlow::WaitUntil(wait_deadline) => {
                *cause = StartCause::ResumeTimeReached {
                    start,
                    requested_resume: *wait_deadline,
                };
                timeout = if *wait_deadline > start {
                    Some(*wait_deadline - start)
                } else {
                    Some(Duration::from_millis(0))
                };
                deadline = Some(*wait_deadline);
            }
        }

        IterationResult {
            wait_start: start,
            deadline,
            timeout,
        }
    }

    pub fn run<F>(mut self, event_handler: F) -> !
    where
        F: 'static
            + FnMut(event::Event<'_, T>, &event_loop::EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let exit_code = self.run_return(event_handler);
        ::std::process::exit(exit_code);
    }

    pub fn run_return<F>(&mut self, mut callback: F) -> i32
    where
        F: FnMut(event::Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        let mut control_flow = ControlFlow::default();
        let mut cause = StartCause::Init;
        let mut pending_redraw = false;

        // run the initial loop iteration
        let mut iter_result = self.single_iteration(
            &mut control_flow,
            None,
            &mut pending_redraw,
            &mut cause,
            &mut callback,
        );

        let exit_code = loop {
            if let ControlFlow::ExitWithCode(code) = control_flow {
                break code;
            }

            let mut timeout = iter_result.timeout;

            // If we already have work to do then we don't want to block on the next poll...
            pending_redraw |= self.redraw_flag.get_and_reset();
            if self.running && (pending_redraw || self.user_events_receiver.has_incoming()) {
                timeout = Some(Duration::from_millis(0))
            }

            let app = self.android_app.clone(); // Don't borrow self as part of poll expression
            app.poll_events(timeout, |poll_event| {
                let mut main_event = None;

                match poll_event {
                    android_activity::PollEvent::Wake => {
                        // In the X11 backend it's noted that too many false-positive wake ups
                        // would cause the event loop to run continuously. They handle this by re-checking
                        // for pending events (assuming they cover all valid reasons for a wake up).
                        //
                        // For now, user_events and redraw_requests are the only reasons to expect
                        // a wake up here so we can ignore the wake up if there are no events/requests.
                        // We also ignore wake ups while suspended.
                        pending_redraw |= self.redraw_flag.get_and_reset();
                        if !self.running
                            || (!pending_redraw && !self.user_events_receiver.has_incoming())
                        {
                            return;
                        }
                    }
                    android_activity::PollEvent::Timeout => {}
                    android_activity::PollEvent::Main(event) => {
                        main_event = Some(event);
                    }
                    unknown_event => {
                        warn!("Unknown poll event {unknown_event:?} (ignored)");
                    }
                }

                let wait_cancelled = iter_result
                    .deadline
                    .map_or(false, |deadline| Instant::now() < deadline);

                if wait_cancelled {
                    cause = StartCause::WaitCancelled {
                        start: iter_result.wait_start,
                        requested_resume: iter_result.deadline,
                    };
                }

                iter_result = self.single_iteration(
                    &mut control_flow,
                    main_event,
                    &mut pending_redraw,
                    &mut cause,
                    &mut callback,
                );
            });
        };

        sticky_exit_callback(
            event::Event::LoopDestroyed,
            self.window_target(),
            &mut control_flow,
            &mut callback,
        );

        exit_code
    }

    pub fn window_target(&self) -> &event_loop::EventLoopWindowTarget<T> {
        &self.window_target
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            user_events_sender: self.user_events_sender.clone(),
            waker: self.android_app.create_waker(),
        }
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
        self.user_events_sender
            .send(event)
            .map_err(|err| event_loop::EventLoopClosed(err.0))?;
        self.waker.wake();
        Ok(())
    }
}

pub struct EventLoopWindowTarget<T: 'static> {
    app: AndroidApp,
    redraw_requester: RedrawRequester,
    _marker: std::marker::PhantomData<T>,
}

impl<T: 'static> EventLoopWindowTarget<T> {
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle::new(self.app.clone()))
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle::new(self.app.clone()));
        v
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Android(AndroidDisplayHandle::empty())
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
pub struct DeviceId;

impl DeviceId {
    pub const fn dummy() -> Self {
        DeviceId
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformSpecificWindowBuilderAttributes;

pub(crate) struct Window {
    app: AndroidApp,
    redraw_requester: RedrawRequester,
}

impl Window {
    pub(crate) fn new<T: 'static>(
        el: &EventLoopWindowTarget<T>,
        _window_attrs: window::WindowAttributes,
        _: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, error::OsError> {
        // FIXME this ignores requested window attributes

        Ok(Self {
            app: el.app.clone(),
            redraw_requester: el.redraw_requester.clone(),
        })
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

    pub fn set_inner_size(&self, _size: Size) {
        warn!("Cannot set window size on Android");
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

    pub fn set_ime_position(&self, _position: Position) {}

    pub fn set_ime_allowed(&self, _allowed: bool) {}

    pub fn set_ime_purpose(&self, _purpose: ImePurpose) {}

    pub fn focus_window(&self) {}

    pub fn request_user_attention(&self, _request_type: Option<window::UserAttentionType>) {}

    pub fn set_cursor_icon(&self, _: window::CursorIcon) {}

    pub fn set_cursor_position(&self, _: Position) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn set_cursor_grab(&self, _: CursorGrabMode) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn set_cursor_visible(&self, _: bool) {}

    pub fn drag_window(&self) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn drag_resize_window(
        &self,
        _direction: ResizeDirection,
    ) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        if let Some(native_window) = self.app.native_window().as_ref() {
            native_window.raw_window_handle()
        } else {
            panic!("Cannot get the native window, it's null and will always be null before Event::Resumed and after Event::Suspended. Make sure you only call this function between those events.");
        }
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Android(AndroidDisplayHandle::empty())
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

    pub fn has_focus(&self) -> bool {
        *HAS_FOCUS.read().unwrap()
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

pub(crate) use crate::icon::NoIcon as PlatformIcon;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MonitorHandle {
    app: AndroidApp,
}
impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        Some(std::cmp::Ordering::Equal)
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
        self.app
            .config()
            .density()
            .map(|dpi| dpi as f64 / 160.0)
            .unwrap_or(1.0)
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        // FIXME no way to get real refresh rate for now.
        None
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        let size = self.size().into();
        // FIXME this is not the real refresh rate
        // (it is guaranteed to support 32 bit color though)
        std::iter::once(VideoMode {
            size,
            bit_depth: 32,
            refresh_rate_millihertz: 60000,
            monitor: self.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoMode {
    size: (u32, u32),
    bit_depth: u16,
    refresh_rate_millihertz: u32,
    monitor: MonitorHandle,
}

impl VideoMode {
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

fn keycode_to_logical(keycode: ndk::event::Keycode, native: NativeKey) -> Key {
    use ndk::event::Keycode::*;

    // The android `Keycode` is sort-of layout dependent. More specifically
    // if I press the Z key using a US layout, then I get KEYCODE_Z,
    // but if I press the same key after switching to a HUN layout, I get
    // KEYCODE_Y.
    //
    // To prevents us from using this value to determine the `physical_key`
    // (also know as winit's `KeyCode`)
    //
    // Unfortunately the documentation says that the scancode values
    // "are not reliable and vary from device to device". Which seems to mean
    // that there's no way to reliably get the physical_key on android.

    match keycode {
        Unknown => Key::Unidentified(native),

        // Can be added on demand
        SoftLeft => Key::Unidentified(native),
        SoftRight => Key::Unidentified(native),

        // Using `BrowserHome` instead of `GoHome` according to
        // https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/key/Key_Values
        Home => Key::BrowserHome,
        Back => Key::BrowserBack,
        Call => Key::Call,
        Endcall => Key::EndCall,

        //-------------------------------------------------------------------------------
        // Reporting unidentified, because the specific character is layout dependent.
        // (I'm not sure though)
        Keycode0 => Key::Unidentified(native),
        Keycode1 => Key::Unidentified(native),
        Keycode2 => Key::Unidentified(native),
        Keycode3 => Key::Unidentified(native),
        Keycode4 => Key::Unidentified(native),
        Keycode5 => Key::Unidentified(native),
        Keycode6 => Key::Unidentified(native),
        Keycode7 => Key::Unidentified(native),
        Keycode8 => Key::Unidentified(native),
        Keycode9 => Key::Unidentified(native),
        Star => Key::Unidentified(native),
        Pound => Key::Unidentified(native),
        A => Key::Unidentified(native),
        B => Key::Unidentified(native),
        C => Key::Unidentified(native),
        D => Key::Unidentified(native),
        E => Key::Unidentified(native),
        F => Key::Unidentified(native),
        G => Key::Unidentified(native),
        H => Key::Unidentified(native),
        I => Key::Unidentified(native),
        J => Key::Unidentified(native),
        K => Key::Unidentified(native),
        L => Key::Unidentified(native),
        M => Key::Unidentified(native),
        N => Key::Unidentified(native),
        O => Key::Unidentified(native),
        P => Key::Unidentified(native),
        Q => Key::Unidentified(native),
        R => Key::Unidentified(native),
        S => Key::Unidentified(native),
        T => Key::Unidentified(native),
        U => Key::Unidentified(native),
        V => Key::Unidentified(native),
        W => Key::Unidentified(native),
        X => Key::Unidentified(native),
        Y => Key::Unidentified(native),
        Z => Key::Unidentified(native),
        Comma => Key::Unidentified(native),
        Period => Key::Unidentified(native),
        Grave => Key::Unidentified(native),
        Minus => Key::Unidentified(native),
        Equals => Key::Unidentified(native),
        LeftBracket => Key::Unidentified(native),
        RightBracket => Key::Unidentified(native),
        Backslash => Key::Unidentified(native),
        Semicolon => Key::Unidentified(native),
        Apostrophe => Key::Unidentified(native),
        Slash => Key::Unidentified(native),
        At => Key::Unidentified(native),
        Plus => Key::Unidentified(native),
        //-------------------------------------------------------------------------------
        DpadUp => Key::ArrowUp,
        DpadDown => Key::ArrowDown,
        DpadLeft => Key::ArrowLeft,
        DpadRight => Key::ArrowRight,
        DpadCenter => Key::Enter,

        VolumeUp => Key::AudioVolumeUp,
        VolumeDown => Key::AudioVolumeDown,
        Power => Key::Power,
        Camera => Key::Camera,
        Clear => Key::Clear,

        AltLeft => Key::Alt,
        AltRight => Key::Alt,
        ShiftLeft => Key::Shift,
        ShiftRight => Key::Shift,
        Tab => Key::Tab,
        Space => Key::Space,
        Sym => Key::Symbol,
        Explorer => Key::LaunchWebBrowser,
        Envelope => Key::LaunchMail,
        Enter => Key::Enter,
        Del => Key::Backspace,

        // According to https://developer.android.com/reference/android/view/KeyEvent#KEYCODE_NUM
        Num => Key::Alt,

        Headsethook => Key::HeadsetHook,
        Focus => Key::CameraFocus,

        Menu => Key::Unidentified(native),

        Notification => Key::Notification,
        Search => Key::BrowserSearch,
        MediaPlayPause => Key::MediaPlayPause,
        MediaStop => Key::MediaStop,
        MediaNext => Key::MediaTrackNext,
        MediaPrevious => Key::MediaTrackPrevious,
        MediaRewind => Key::MediaRewind,
        MediaFastForward => Key::MediaFastForward,
        Mute => Key::MicrophoneVolumeMute,
        PageUp => Key::PageUp,
        PageDown => Key::PageDown,
        Pictsymbols => Key::Unidentified(native),
        SwitchCharset => Key::Unidentified(native),

        // -----------------------------------------------------------------
        // Gamepad events should be exposed through a separate API, not
        // keyboard events
        ButtonA => Key::Unidentified(native),
        ButtonB => Key::Unidentified(native),
        ButtonC => Key::Unidentified(native),
        ButtonX => Key::Unidentified(native),
        ButtonY => Key::Unidentified(native),
        ButtonZ => Key::Unidentified(native),
        ButtonL1 => Key::Unidentified(native),
        ButtonR1 => Key::Unidentified(native),
        ButtonL2 => Key::Unidentified(native),
        ButtonR2 => Key::Unidentified(native),
        ButtonThumbl => Key::Unidentified(native),
        ButtonThumbr => Key::Unidentified(native),
        ButtonStart => Key::Unidentified(native),
        ButtonSelect => Key::Unidentified(native),
        ButtonMode => Key::Unidentified(native),
        // -----------------------------------------------------------------
        Escape => Key::Escape,
        ForwardDel => Key::Delete,
        CtrlLeft => Key::Control,
        CtrlRight => Key::Control,
        CapsLock => Key::CapsLock,
        ScrollLock => Key::ScrollLock,
        MetaLeft => Key::Super,
        MetaRight => Key::Super,
        Function => Key::Fn,
        Sysrq => Key::PrintScreen,
        Break => Key::Pause,
        MoveHome => Key::Home,
        MoveEnd => Key::End,
        Insert => Key::Insert,
        Forward => Key::BrowserForward,
        MediaPlay => Key::MediaPlay,
        MediaPause => Key::MediaPause,
        MediaClose => Key::MediaClose,
        MediaEject => Key::Eject,
        MediaRecord => Key::MediaRecord,
        F1 => Key::F1,
        F2 => Key::F2,
        F3 => Key::F3,
        F4 => Key::F4,
        F5 => Key::F5,
        F6 => Key::F6,
        F7 => Key::F7,
        F8 => Key::F8,
        F9 => Key::F9,
        F10 => Key::F10,
        F11 => Key::F11,
        F12 => Key::F12,
        NumLock => Key::NumLock,
        Numpad0 => Key::Unidentified(native),
        Numpad1 => Key::Unidentified(native),
        Numpad2 => Key::Unidentified(native),
        Numpad3 => Key::Unidentified(native),
        Numpad4 => Key::Unidentified(native),
        Numpad5 => Key::Unidentified(native),
        Numpad6 => Key::Unidentified(native),
        Numpad7 => Key::Unidentified(native),
        Numpad8 => Key::Unidentified(native),
        Numpad9 => Key::Unidentified(native),
        NumpadDivide => Key::Unidentified(native),
        NumpadMultiply => Key::Unidentified(native),
        NumpadSubtract => Key::Unidentified(native),
        NumpadAdd => Key::Unidentified(native),
        NumpadDot => Key::Unidentified(native),
        NumpadComma => Key::Unidentified(native),
        NumpadEnter => Key::Unidentified(native),
        NumpadEquals => Key::Unidentified(native),
        NumpadLeftParen => Key::Unidentified(native),
        NumpadRightParen => Key::Unidentified(native),

        VolumeMute => Key::AudioVolumeMute,
        Info => Key::Info,
        ChannelUp => Key::ChannelUp,
        ChannelDown => Key::ChannelDown,
        ZoomIn => Key::ZoomIn,
        ZoomOut => Key::ZoomOut,
        Tv => Key::TV,
        Window => Key::Unidentified(native),
        Guide => Key::Guide,
        Dvr => Key::DVR,
        Bookmark => Key::BrowserFavorites,
        Captions => Key::ClosedCaptionToggle,
        Settings => Key::Settings,
        TvPower => Key::TVPower,
        TvInput => Key::TVInput,
        StbPower => Key::STBPower,
        StbInput => Key::STBInput,
        AvrPower => Key::AVRPower,
        AvrInput => Key::AVRInput,
        ProgRed => Key::ColorF0Red,
        ProgGreen => Key::ColorF1Green,
        ProgYellow => Key::ColorF2Yellow,
        ProgBlue => Key::ColorF3Blue,
        AppSwitch => Key::AppSwitch,
        Button1 => Key::Unidentified(native),
        Button2 => Key::Unidentified(native),
        Button3 => Key::Unidentified(native),
        Button4 => Key::Unidentified(native),
        Button5 => Key::Unidentified(native),
        Button6 => Key::Unidentified(native),
        Button7 => Key::Unidentified(native),
        Button8 => Key::Unidentified(native),
        Button9 => Key::Unidentified(native),
        Button10 => Key::Unidentified(native),
        Button11 => Key::Unidentified(native),
        Button12 => Key::Unidentified(native),
        Button13 => Key::Unidentified(native),
        Button14 => Key::Unidentified(native),
        Button15 => Key::Unidentified(native),
        Button16 => Key::Unidentified(native),
        LanguageSwitch => Key::GroupNext,
        MannerMode => Key::MannerMode,
        Keycode3dMode => Key::TV3DMode,
        Contacts => Key::LaunchContacts,
        Calendar => Key::LaunchCalendar,
        Music => Key::LaunchMusicPlayer,
        Calculator => Key::LaunchApplication2,
        ZenkakuHankaku => Key::ZenkakuHankaku,
        Eisu => Key::Eisu,
        Muhenkan => Key::NonConvert,
        Henkan => Key::Convert,
        KatakanaHiragana => Key::HiraganaKatakana,
        Yen => Key::Unidentified(native),
        Ro => Key::Unidentified(native),
        Kana => Key::KanjiMode,
        Assist => Key::Unidentified(native),
        BrightnessDown => Key::BrightnessDown,
        BrightnessUp => Key::BrightnessUp,
        MediaAudioTrack => Key::MediaAudioTrack,
        Sleep => Key::Standby,
        Wakeup => Key::WakeUp,
        Pairing => Key::Pairing,
        MediaTopMenu => Key::MediaTopMenu,
        Keycode11 => Key::Unidentified(native),
        Keycode12 => Key::Unidentified(native),
        LastChannel => Key::MediaLast,
        TvDataService => Key::TVDataService,
        VoiceAssist => Key::VoiceDial,
        TvRadioService => Key::TVRadioService,
        TvTeletext => Key::Teletext,
        TvNumberEntry => Key::TVNumberEntry,
        TvTerrestrialAnalog => Key::TVTerrestrialAnalog,
        TvTerrestrialDigital => Key::TVTerrestrialDigital,
        TvSatellite => Key::TVSatellite,
        TvSatelliteBs => Key::TVSatelliteBS,
        TvSatelliteCs => Key::TVSatelliteCS,
        TvSatelliteService => Key::TVSatelliteToggle,
        TvNetwork => Key::TVNetwork,
        TvAntennaCable => Key::TVAntennaCable,
        TvInputHdmi1 => Key::TVInputHDMI1,
        TvInputHdmi2 => Key::TVInputHDMI2,
        TvInputHdmi3 => Key::TVInputHDMI3,
        TvInputHdmi4 => Key::TVInputHDMI4,
        TvInputComposite1 => Key::TVInputComposite1,
        TvInputComposite2 => Key::TVInputComposite2,
        TvInputComponent1 => Key::TVInputComponent1,
        TvInputComponent2 => Key::TVInputComponent2,
        TvInputVga1 => Key::TVInputVGA1,
        TvAudioDescription => Key::TVAudioDescription,
        TvAudioDescriptionMixUp => Key::TVAudioDescriptionMixUp,
        TvAudioDescriptionMixDown => Key::TVAudioDescriptionMixDown,
        TvZoomMode => Key::ZoomToggle,
        TvContentsMenu => Key::TVContentsMenu,
        TvMediaContextMenu => Key::TVMediaContext,
        TvTimerProgramming => Key::TVTimer,
        Help => Key::Help,
        NavigatePrevious => Key::NavigatePrevious,
        NavigateNext => Key::NavigateNext,
        NavigateIn => Key::NavigateIn,
        NavigateOut => Key::NavigateOut,
        StemPrimary => Key::Unidentified(native),
        Stem1 => Key::Unidentified(native),
        Stem2 => Key::Unidentified(native),
        Stem3 => Key::Unidentified(native),
        DpadUpLeft => Key::Unidentified(native),
        DpadDownLeft => Key::Unidentified(native),
        DpadUpRight => Key::Unidentified(native),
        DpadDownRight => Key::Unidentified(native),
        MediaSkipForward => Key::MediaSkipForward,
        MediaSkipBackward => Key::MediaSkipBackward,
        MediaStepForward => Key::MediaStepForward,
        MediaStepBackward => Key::MediaStepBackward,
        SoftSleep => Key::Unidentified(native),
        Cut => Key::Cut,
        Copy => Key::Copy,
        Paste => Key::Paste,
        SystemNavigationUp => Key::Unidentified(native),
        SystemNavigationDown => Key::Unidentified(native),
        SystemNavigationLeft => Key::Unidentified(native),
        SystemNavigationRight => Key::Unidentified(native),
        AllApps => Key::Unidentified(native),
        Refresh => Key::BrowserRefresh,
        ThumbsUp => Key::Unidentified(native),
        ThumbsDown => Key::Unidentified(native),
        ProfileSwitch => Key::Unidentified(native),
    }
}

fn keycode_to_location(keycode: ndk::event::Keycode) -> KeyLocation {
    use ndk::event::Keycode::*;

    match keycode {
        AltLeft => KeyLocation::Left,
        AltRight => KeyLocation::Right,
        ShiftLeft => KeyLocation::Left,
        ShiftRight => KeyLocation::Right,

        // According to https://developer.android.com/reference/android/view/KeyEvent#KEYCODE_NUM
        Num => KeyLocation::Left,

        CtrlLeft => KeyLocation::Left,
        CtrlRight => KeyLocation::Right,
        MetaLeft => KeyLocation::Left,
        MetaRight => KeyLocation::Right,

        NumLock => KeyLocation::Numpad,
        Numpad0 => KeyLocation::Numpad,
        Numpad1 => KeyLocation::Numpad,
        Numpad2 => KeyLocation::Numpad,
        Numpad3 => KeyLocation::Numpad,
        Numpad4 => KeyLocation::Numpad,
        Numpad5 => KeyLocation::Numpad,
        Numpad6 => KeyLocation::Numpad,
        Numpad7 => KeyLocation::Numpad,
        Numpad8 => KeyLocation::Numpad,
        Numpad9 => KeyLocation::Numpad,
        NumpadDivide => KeyLocation::Numpad,
        NumpadMultiply => KeyLocation::Numpad,
        NumpadSubtract => KeyLocation::Numpad,
        NumpadAdd => KeyLocation::Numpad,
        NumpadDot => KeyLocation::Numpad,
        NumpadComma => KeyLocation::Numpad,
        NumpadEnter => KeyLocation::Numpad,
        NumpadEquals => KeyLocation::Numpad,
        NumpadLeftParen => KeyLocation::Numpad,
        NumpadRightParen => KeyLocation::Numpad,

        _ => KeyLocation::Standard,
    }
}
