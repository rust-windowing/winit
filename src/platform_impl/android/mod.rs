#![cfg(target_os = "android")]

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error, event,
    event_loop::{self, ControlFlow},
    monitor, window,
};
use ndk::{
    configuration::Configuration,
    event::{InputEvent, KeyAction, MotionAction},
    looper::{ForeignLooper, Poll, ThreadLooper},
};
use ndk_glue::{Event, Rect};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

lazy_static! {
    static ref CONFIG: RwLock<Configuration> = RwLock::new(Configuration::new());
    // If this is `Some()` a `Poll::Wake` is considered an `EventSource::Internal` with the event
    // contained in the `Option`. The event is moved outside of the `Option` replacing it with a
    // `None`.
    //
    // This allows us to inject event into the event loop without going through `ndk-glue` and
    // calling unsafe function that should only be called by Android.
    static ref INTERNAL_EVENT: RwLock<Option<InternalEvent>> = RwLock::new(None);
}

enum InternalEvent {
    RedrawRequested,
}

enum EventSource {
    Callback,
    InputQueue,
    User,
    Internal(InternalEvent),
}

fn poll(poll: Poll) -> Option<EventSource> {
    match poll {
        Poll::Event { ident, .. } => match ident {
            ndk_glue::NDK_GLUE_LOOPER_EVENT_PIPE_IDENT => Some(EventSource::Callback),
            ndk_glue::NDK_GLUE_LOOPER_INPUT_QUEUE_IDENT => Some(EventSource::InputQueue),
            _ => unreachable!(),
        },
        Poll::Timeout => None,
        Poll::Wake => Some(
            INTERNAL_EVENT
                .write()
                .unwrap()
                .take()
                .map_or(EventSource::User, EventSource::Internal),
        ),
        Poll::Callback => unreachable!(),
    }
}

pub struct EventLoop<T: 'static> {
    window_target: event_loop::EventLoopWindowTarget<T>,
    user_queue: Arc<Mutex<VecDeque<T>>>,
    first_event: Option<EventSource>,
    start_cause: event::StartCause,
    looper: ThreadLooper,
    running: bool,
}

macro_rules! call_event_handler {
    ( $event_handler:expr, $window_target:expr, $cf:expr, $event:expr ) => {{
        if $cf != ControlFlow::Exit {
            $event_handler($event, $window_target, &mut $cf);
        } else {
            $event_handler($event, $window_target, &mut ControlFlow::Exit);
        }
    }};
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> Self {
        Self {
            window_target: event_loop::EventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    _marker: std::marker::PhantomData,
                },
                _marker: std::marker::PhantomData,
            },
            user_queue: Default::default(),
            first_event: None,
            start_cause: event::StartCause::Init,
            looper: ThreadLooper::for_thread().unwrap(),
            running: false,
        }
    }

    pub fn run<F>(mut self, event_handler: F) -> !
    where
        F: 'static
            + FnMut(event::Event<'_, T>, &event_loop::EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        self.run_return(event_handler);
        ::std::process::exit(0);
    }

    pub fn run_return<F>(&mut self, mut event_handler: F)
    where
        F: FnMut(event::Event<'_, T>, &event_loop::EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let mut control_flow = ControlFlow::default();

        'event_loop: loop {
            call_event_handler!(
                event_handler,
                self.window_target(),
                control_flow,
                event::Event::NewEvents(self.start_cause)
            );

            let mut redraw = false;
            let mut resized = false;

            match self.first_event.take() {
                Some(EventSource::Callback) => match ndk_glue::poll_events().unwrap() {
                    Event::WindowCreated => {
                        call_event_handler!(
                            event_handler,
                            self.window_target(),
                            control_flow,
                            event::Event::Resumed
                        );
                    }
                    Event::WindowResized => resized = true,
                    Event::WindowRedrawNeeded => redraw = true,
                    Event::WindowDestroyed => {
                        call_event_handler!(
                            event_handler,
                            self.window_target(),
                            control_flow,
                            event::Event::Suspended
                        );
                    }
                    Event::Pause => self.running = false,
                    Event::Resume => self.running = true,
                    Event::ConfigChanged => {
                        let am = ndk_glue::native_activity().asset_manager();
                        let config = Configuration::from_asset_manager(&am);
                        let old_scale_factor = MonitorHandle.scale_factor();
                        *CONFIG.write().unwrap() = config;
                        let scale_factor = MonitorHandle.scale_factor();
                        if (scale_factor - old_scale_factor).abs() < f64::EPSILON {
                            let mut size = MonitorHandle.size();
                            let event = event::Event::WindowEvent {
                                window_id: window::WindowId(WindowId),
                                event: event::WindowEvent::ScaleFactorChanged {
                                    new_inner_size: &mut size,
                                    scale_factor,
                                },
                            };
                            call_event_handler!(
                                event_handler,
                                self.window_target(),
                                control_flow,
                                event
                            );
                        }
                    }
                    Event::WindowHasFocus => {
                        call_event_handler!(
                            event_handler,
                            self.window_target(),
                            control_flow,
                            event::Event::WindowEvent {
                                window_id: window::WindowId(WindowId),
                                event: event::WindowEvent::Focused(true),
                            }
                        );
                    }
                    Event::WindowLostFocus => {
                        call_event_handler!(
                            event_handler,
                            self.window_target(),
                            control_flow,
                            event::Event::WindowEvent {
                                window_id: window::WindowId(WindowId),
                                event: event::WindowEvent::Focused(false),
                            }
                        );
                    }
                    _ => {}
                },
                Some(EventSource::InputQueue) => {
                    if let Some(input_queue) = ndk_glue::input_queue().as_ref() {
                        while let Some(event) = input_queue.get_event() {
                            if let Some(event) = input_queue.pre_dispatch(event) {
                                let mut handled = true;
                                let window_id = window::WindowId(WindowId);
                                let device_id = event::DeviceId(DeviceId);
                                match &event {
                                    InputEvent::MotionEvent(motion_event) => {
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
                                                handled = false;
                                                None // TODO mouse events
                                            }
                                        };
                                        if let Some(phase) = phase {
                                            let pointers: Box<
                                                dyn Iterator<Item = ndk::event::Pointer<'_>>,
                                            > = match phase {
                                                event::TouchPhase::Started
                                                | event::TouchPhase::Ended => Box::new(
                                                    std::iter::once(motion_event.pointer_at_index(
                                                        motion_event.pointer_index(),
                                                    )),
                                                ),
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
                                                call_event_handler!(
                                                    event_handler,
                                                    self.window_target(),
                                                    control_flow,
                                                    event
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
                                        #[allow(deprecated)]
                                        let event = event::Event::WindowEvent {
                                            window_id,
                                            event: event::WindowEvent::KeyboardInput {
                                                device_id,
                                                input: event::KeyboardInput {
                                                    scancode: key.scan_code() as u32,
                                                    state,
                                                    virtual_keycode: None,
                                                    modifiers: event::ModifiersState::default(),
                                                },
                                                is_synthetic: false,
                                            },
                                        };
                                        call_event_handler!(
                                            event_handler,
                                            self.window_target(),
                                            control_flow,
                                            event
                                        );
                                    }
                                };
                                input_queue.finish_event(event, handled);
                            }
                        }
                    }
                }
                Some(EventSource::User) => {
                    let mut user_queue = self.user_queue.lock().unwrap();
                    while let Some(event) = user_queue.pop_front() {
                        call_event_handler!(
                            event_handler,
                            self.window_target(),
                            control_flow,
                            event::Event::UserEvent(event)
                        );
                    }
                }
                Some(EventSource::Internal(internal)) => match internal {
                    InternalEvent::RedrawRequested => redraw = true,
                },
                None => {}
            }

            call_event_handler!(
                event_handler,
                self.window_target(),
                control_flow,
                event::Event::MainEventsCleared
            );

            if resized && self.running {
                let size = MonitorHandle.size();
                let event = event::Event::WindowEvent {
                    window_id: window::WindowId(WindowId),
                    event: event::WindowEvent::Resized(size),
                };
                call_event_handler!(event_handler, self.window_target(), control_flow, event);
            }

            if redraw && self.running {
                let event = event::Event::RedrawRequested(window::WindowId(WindowId));
                call_event_handler!(event_handler, self.window_target(), control_flow, event);
            }

            call_event_handler!(
                event_handler,
                self.window_target(),
                control_flow,
                event::Event::RedrawEventsCleared
            );

            match control_flow {
                ControlFlow::Exit => {
                    self.first_event = poll(
                        self.looper
                            .poll_once_timeout(Duration::from_millis(0))
                            .unwrap(),
                    );
                    self.start_cause = event::StartCause::WaitCancelled {
                        start: Instant::now(),
                        requested_resume: None,
                    };
                    break 'event_loop;
                }
                ControlFlow::Poll => {
                    self.first_event = poll(
                        self.looper
                            .poll_all_timeout(Duration::from_millis(0))
                            .unwrap(),
                    );
                    self.start_cause = event::StartCause::Poll;
                }
                ControlFlow::Wait => {
                    self.first_event = poll(self.looper.poll_all().unwrap());
                    self.start_cause = event::StartCause::WaitCancelled {
                        start: Instant::now(),
                        requested_resume: None,
                    }
                }
                ControlFlow::WaitUntil(instant) => {
                    let start = Instant::now();
                    let duration = if instant <= start {
                        Duration::default()
                    } else {
                        instant - start
                    };
                    self.first_event = poll(self.looper.poll_all_timeout(duration).unwrap());
                    self.start_cause = if self.first_event.is_some() {
                        event::StartCause::WaitCancelled {
                            start,
                            requested_resume: Some(instant),
                        }
                    } else {
                        event::StartCause::ResumeTimeReached {
                            start,
                            requested_resume: instant,
                        }
                    }
                }
            }
        }
    }

    pub fn window_target(&self) -> &event_loop::EventLoopWindowTarget<T> {
        &self.window_target
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            queue: self.user_queue.clone(),
            looper: ForeignLooper::for_thread().expect("called from event loop thread"),
        }
    }
}

pub struct EventLoopProxy<T: 'static> {
    queue: Arc<Mutex<VecDeque<T>>>,
    looper: ForeignLooper,
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), event_loop::EventLoopClosed<T>> {
        self.queue.lock().unwrap().push_back(event);
        self.looper.wake();
        Ok(())
    }
}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy {
            queue: self.queue.clone(),
            looper: self.looper.clone(),
        }
    }
}

pub struct EventLoopWindowTarget<T: 'static> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: 'static> EventLoopWindowTarget<T> {
    pub fn primary_monitor(&self) -> Option<monitor::MonitorHandle> {
        Some(monitor::MonitorHandle {
            inner: MonitorHandle,
        })
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle);
        v
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WindowId;

impl WindowId {
    pub fn dummy() -> Self {
        WindowId
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceId;

impl DeviceId {
    pub fn dummy() -> Self {
        DeviceId
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformSpecificWindowBuilderAttributes;

pub struct Window;

impl Window {
    pub fn new<T: 'static>(
        _el: &EventLoopWindowTarget<T>,
        _window_attrs: window::WindowAttributes,
        _: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, error::OsError> {
        // FIXME this ignores requested window attributes
        Ok(Self)
    }

    pub fn id(&self) -> WindowId {
        WindowId
    }

    pub fn primary_monitor(&self) -> Option<monitor::MonitorHandle> {
        Some(monitor::MonitorHandle {
            inner: MonitorHandle,
        })
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle);
        v
    }

    pub fn current_monitor(&self) -> Option<monitor::MonitorHandle> {
        Some(monitor::MonitorHandle {
            inner: MonitorHandle,
        })
    }

    pub fn scale_factor(&self) -> f64 {
        MonitorHandle.scale_factor()
    }

    pub fn request_redraw(&self) {
        *INTERNAL_EVENT.write().unwrap() = Some(InternalEvent::RedrawRequested);
        ForeignLooper::for_thread().unwrap().wake();
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
        MonitorHandle.size()
    }

    pub fn set_min_inner_size(&self, _: Option<Size>) {}

    pub fn set_max_inner_size(&self, _: Option<Size>) {}

    pub fn set_title(&self, _title: &str) {}

    pub fn set_visible(&self, _visibility: bool) {}

    pub fn set_resizable(&self, _resizeable: bool) {}

    pub fn set_minimized(&self, _minimized: bool) {}

    pub fn set_maximized(&self, _maximized: bool) {}

    pub fn is_maximized(&self) -> bool {
        false
    }

    pub fn set_fullscreen(&self, _monitor: Option<window::Fullscreen>) {
        warn!("Cannot set fullscreen on Android");
    }

    pub fn fullscreen(&self) -> Option<window::Fullscreen> {
        None
    }

    pub fn set_decorations(&self, _decorations: bool) {}

    pub fn set_always_on_top(&self, _always_on_top: bool) {}

    pub fn set_window_icon(&self, _window_icon: Option<crate::icon::Icon>) {}

    pub fn set_ime_position(&self, _position: Position) {}

    pub fn focus_window(&self) {}

    pub fn request_user_attention(&self, _request_type: Option<window::UserAttentionType>) {}

    pub fn set_cursor_icon(&self, _: window::CursorIcon) {}

    pub fn set_cursor_position(&self, _: Position) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn set_cursor_grab(&self, _: bool) -> Result<(), error::ExternalError> {
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

    pub fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        let a_native_window = if let Some(native_window) = ndk_glue::native_window().as_ref() {
            unsafe { native_window.ptr().as_mut() as *mut _ as *mut _ }
        } else {
            panic!("Cannot get the native window, it's null and will always be null before Event::Resumed and after Event::Suspended. Make sure you only call this function between those events.");
        };
        let mut handle = raw_window_handle::android::AndroidHandle::empty();
        handle.a_native_window = a_native_window;
        raw_window_handle::RawWindowHandle::Android(handle)
    }

    pub fn config(&self) -> Configuration {
        CONFIG.read().unwrap().clone()
    }

    pub fn content_rect(&self) -> Rect {
        ndk_glue::content_rect()
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

pub(crate) use crate::icon::NoIcon as PlatformIcon;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MonitorHandle;

impl MonitorHandle {
    pub fn name(&self) -> Option<String> {
        Some("Android Device".to_owned())
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        if let Some(native_window) = ndk_glue::native_window().as_ref() {
            let width = native_window.width() as _;
            let height = native_window.height() as _;
            PhysicalSize::new(width, height)
        } else {
            PhysicalSize::new(0, 0)
        }
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        (0, 0).into()
    }

    pub fn scale_factor(&self) -> f64 {
        let config = CONFIG.read().unwrap();
        config
            .density()
            .map(|dpi| dpi as f64 / 160.0)
            .unwrap_or(1.0)
    }

    pub fn video_modes(&self) -> impl Iterator<Item = monitor::VideoMode> {
        let size = self.size().into();
        let mut v = Vec::new();
        // FIXME this is not the real refresh rate
        // (it is guarunteed to support 32 bit color though)
        v.push(monitor::VideoMode {
            video_mode: VideoMode {
                size,
                bit_depth: 32,
                refresh_rate: 60,
                monitor: self.clone(),
            },
        });
        v.into_iter()
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoMode {
    size: (u32, u32),
    bit_depth: u16,
    refresh_rate: u16,
    monitor: MonitorHandle,
}

impl VideoMode {
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size.into()
    }

    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    pub fn refresh_rate(&self) -> u16 {
        self.refresh_rate
    }

    pub fn monitor(&self) -> monitor::MonitorHandle {
        monitor::MonitorHandle {
            inner: self.monitor.clone(),
        }
    }
}
