#![cfg(target_os = "android")]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use crate::error;
use crate::event;
use crate::event_loop::{self, ControlFlow};
use crate::monitor;
use crate::window;

use android_ndk::android_app::{AndroidApp, Cmd};
use android_ndk::event::{InputEvent, MotionAction};
use android_ndk::looper::{ForeignLooper, Poll, ThreadLooper};
use android_ndk_sys::native_app_glue;

#[link(name = "GLESv2")]
#[link(name = "EGL")]
extern "C" {}

pub enum Event {
    Cmd,
    Input,
    User,
}

fn convert(poll: Poll) -> Option<Event> {
    match poll {
        Poll::Event { data, .. } => {
            assert!(!data.is_null());
            let source = unsafe { &*(data as *const native_app_glue::android_poll_source) };
            Some(match source.id {
                native_app_glue::LOOPER_ID_MAIN => Event::Cmd,
                native_app_glue::LOOPER_ID_INPUT => Event::Input,
                _ => unreachable!(),
            })
        }
        Poll::Timeout => None,
        Poll::Wake => Some(Event::User),
        Poll::Callback => unreachable!(),
    }
}

pub struct EventLoop<T: 'static> {
    window_target: event_loop::EventLoopWindowTarget<T>,
    user_queue: Arc<Mutex<VecDeque<T>>>,
    suspend_callback: Rc<RefCell<Option<Box<dyn Fn(bool) -> ()>>>>,
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> Self {
        let suspend_callback = Rc::new(RefCell::new(None));
        Self {
            window_target: event_loop::EventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    suspend_callback: suspend_callback.clone(),
                    _marker: std::marker::PhantomData,
                },
                _marker: std::marker::PhantomData,
            },
            user_queue: Default::default(),
            suspend_callback,
        }
    }

    pub fn run<F>(self, mut event_handler: F) -> !
    where
        F: 'static
            + FnMut(event::Event<T>, &event_loop::EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let mut cf = ControlFlow::default();

        let mut start_cause = event::StartCause::Init;
        let mut first_event = None;

        let mut android_app = unsafe { AndroidApp::from_ptr(android_glue::get_android_app()) };
        let looper = ThreadLooper::for_thread().unwrap();

        let mut running = false;
        let mut prev_size = MonitorHandle.size();
        let mut redraw = 0;

        loop {
            event_handler(
                event::Event::NewEvents(start_cause),
                self.window_target(),
                &mut cf,
            );

            match first_event.take() {
                Some(Event::Cmd) => {
                    android_app.handle_cmd(|_, cmd| match cmd {
                        // NOTE: Commands WindowResized and WindowRedrawNeeded
                        // are not used in the android ndk glue. ConfigChanged
                        // is unreliable way of detecting orientation changes
                        // because the event is fired before ANativeWindow
                        // updates it's width and height. It also fires when
                        // the phone is rotated 180 degrees even though no
                        // redraw is required.
                        Cmd::InitWindow => {
                            if let Some(cb) = self.suspend_callback.borrow().as_ref() {
                                (*cb)(false);
                            }
                            redraw += 1;
                            event_handler(event::Event::Resumed, self.window_target(), &mut cf);
                        }
                        Cmd::TermWindow => {
                            if let Some(cb) = self.suspend_callback.borrow().as_ref() {
                                (*cb)(true);
                            }
                            event_handler(event::Event::Suspended, self.window_target(), &mut cf);
                        }
                        Cmd::ConfigChanged => {
                            redraw += 10;
                        }
                        Cmd::Pause => {
                            running = false;
                        }
                        Cmd::Resume => {
                            running = true;
                        }
                        cmd => println!("{:?}", cmd),
                    });
                }
                Some(Event::Input) => {
                    let input_queue = android_app
                        .input_queue()
                        .expect("native_app_glue set the input_queue field");
                    while let Some(event) = input_queue.get_event() {
                        if let Some(event) = input_queue.pre_dispatch(event) {
                            let window_id = window::WindowId(WindowId);
                            let device_id = event::DeviceId(DeviceId);
                            match &event {
                                InputEvent::MotionEvent(motion_event) => {
                                    let phase = match motion_event.action() {
                                        MotionAction::Down => Some(event::TouchPhase::Started),
                                        MotionAction::Up => Some(event::TouchPhase::Ended),
                                        MotionAction::Move => Some(event::TouchPhase::Moved),
                                        MotionAction::Cancel => Some(event::TouchPhase::Cancelled),
                                        _ => None, // TODO mouse events
                                    };
                                    let pointer = motion_event.pointer_at_index(0);
                                    let position = PhysicalPosition {
                                        x: pointer.x() as f64,
                                        y: pointer.y() as f64,
                                    };
                                    let dpi = MonitorHandle.hidpi_factor();
                                    let location = LogicalPosition::from_physical(position, dpi);

                                    if let Some(phase) = phase {
                                        let event = event::Event::WindowEvent {
                                            window_id,
                                            event: event::WindowEvent::Touch(event::Touch {
                                                device_id,
                                                phase,
                                                location,
                                                id: 0,
                                                force: None,
                                            }),
                                        };
                                        event_handler(event, self.window_target(), &mut cf);
                                    }
                                }
                                InputEvent::KeyEvent(_) => {} // TODO
                            };
                            input_queue.finish_event(event, true);
                        }
                    }
                }
                _ => {}
            }

            let mut user_queue = self.user_queue.lock().unwrap();
            while let Some(event) = user_queue.pop_front() {
                event_handler(
                    event::Event::UserEvent(event),
                    self.window_target(),
                    &mut cf,
                );
            }

            let new_size = MonitorHandle.size();
            if prev_size != new_size {
                let size = LogicalSize::from_physical(new_size, MonitorHandle.hidpi_factor());
                let event = event::Event::WindowEvent {
                    window_id: window::WindowId(WindowId),
                    event: event::WindowEvent::Resized(size),
                };
                event_handler(event, self.window_target(), &mut cf);
                prev_size = new_size;
            }

            event_handler(
                event::Event::MainEventsCleared,
                self.window_target(),
                &mut cf,
            );

            if running && redraw > 0 {
                let event = event::Event::RedrawRequested(window::WindowId(WindowId));
                event_handler(event, self.window_target(), &mut cf);
                redraw -= 1;
            }

            event_handler(
                event::Event::RedrawEventsCleared,
                self.window_target(),
                &mut cf,
            );

            if redraw > 0 {
                if cf == ControlFlow::Wait {
                    let until = Instant::now() + Duration::from_millis(10);
                    cf = ControlFlow::WaitUntil(until);
                }
            }

            match cf {
                ControlFlow::Exit => panic!(),
                ControlFlow::Poll => {
                    start_cause = event::StartCause::Poll;
                }
                ControlFlow::Wait => {
                    first_event = convert(looper.poll_all().unwrap());
                    start_cause = event::StartCause::WaitCancelled {
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
                    first_event = convert(looper.poll_all_timeout(duration).unwrap());
                    start_cause = if first_event.is_some() {
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

    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(self.primary_monitor());
        v
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
    suspend_callback: Rc<RefCell<Option<Box<dyn Fn(bool) -> ()>>>>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: 'static> EventLoopWindowTarget<T> {
    pub fn set_suspend_callback(&self, cb: Option<Box<dyn Fn(bool) -> ()>>) {
        *self.suspend_callback.borrow_mut() = cb;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId;

impl WindowId {
    pub fn dummy() -> Self {
        WindowId
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

impl DeviceId {
    pub fn dummy() -> Self {
        DeviceId
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MonitorHandle;

impl MonitorHandle {
    pub fn name(&self) -> Option<String> {
        Some("Android Device".to_owned())
    }

    pub fn size(&self) -> PhysicalSize {
        let android_app = unsafe { AndroidApp::from_ptr(android_glue::get_android_app()) };
        if let Some(native_window) = android_app.native_window() {
            let width = native_window.width() as f64;
            let height = native_window.height() as f64;
            PhysicalSize::new(width, height)
        } else {
            PhysicalSize::new(0.0, 0.0)
        }
    }

    pub fn position(&self) -> PhysicalPosition {
        (0, 0).into()
    }

    pub fn hidpi_factor(&self) -> f64 {
        let android_app = unsafe { AndroidApp::from_ptr(android_glue::get_android_app()) };
        android_app
            .config()
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

    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle);
        v
    }

    pub fn current_monitor(&self) -> monitor::MonitorHandle {
        monitor::MonitorHandle {
            inner: MonitorHandle,
        }
    }

    pub fn hidpi_factor(&self) -> f64 {
        MonitorHandle.hidpi_factor()
    }

    pub fn request_redraw(&self) {
        // TODO
    }
}

// FIXME: Most of these functions need to use APIs that relate to multi-window (such as split
// screen, Samsung's floating windows, etc.). They currently don't.
impl Window {
    pub fn inner_position(&self) -> Result<LogicalPosition, error::NotSupportedError> {
        Err(error::NotSupportedError::new())
    }

    pub fn outer_position(&self) -> Result<LogicalPosition, error::NotSupportedError> {
        Err(error::NotSupportedError::new())
    }

    pub fn set_outer_position(&self, _position: LogicalPosition) {
        // no effect
    }

    pub fn inner_size(&self) -> LogicalSize {
        // TODO need to subtract system bar
        self.outer_size()
    }

    pub fn set_inner_size(&self, _size: LogicalSize) {
        panic!("Cannot set window size on Android");
    }

    pub fn outer_size(&self) -> LogicalSize {
        let size = MonitorHandle.size();
        LogicalSize::from_physical(size, self.hidpi_factor())
    }

    pub fn set_min_inner_size(&self, _: Option<LogicalSize>) {
        // no effect
    }

    pub fn set_max_inner_size(&self, _: Option<LogicalSize>) {
        // no effect
    }

    pub fn set_title(&self, _title: &str) {
        // TODO there's probably a way to do this
        // no effect
    }

    pub fn set_visible(&self, _visibility: bool) {
        // no effect
    }

    pub fn set_resizable(&self, _resizeable: bool) {
        // no effect
        // Should probably have an effect with multi-windows though
    }

    pub fn set_minimized(&self, _minimized: bool) {
        // no effect
    }

    pub fn set_maximized(&self, _maximized: bool) {
        // no effect
    }

    pub fn set_fullscreen(&self, _monitor: Option<window::Fullscreen>) {
        // no effect
    }

    pub fn fullscreen(&self) -> Option<window::Fullscreen> {
        Some(window::Fullscreen::Borderless(monitor::MonitorHandle {
            inner: MonitorHandle,
        }))
    }

    pub fn set_decorations(&self, _decorations: bool) {
        // TODO
    }

    pub fn set_always_on_top(&self, _always_on_top: bool) {
        // no effect
    }

    pub fn set_window_icon(&self, _window_icon: Option<crate::icon::Icon>) {
        // no effect
    }

    pub fn set_ime_position(&self, _position: LogicalPosition) {
        // no effect
        // What is an IME candidate box?
    }
}

impl Window {
    pub fn set_cursor_icon(&self, _: window::CursorIcon) {
        // no effect
    }

    pub fn set_cursor_position(&self, _: LogicalPosition) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn set_cursor_grab(&self, _: bool) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn set_cursor_visible(&self, _: bool) {
        // no effect
    }

    #[inline]
    pub fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        let mut handle = raw_window_handle::android::AndroidHandle::empty();
        handle.a_native_window =
            unsafe { android_glue::get_android_app().as_ref() }.window as *mut _;
        raw_window_handle::RawWindowHandle::Android(handle)
    }
}

#[derive(Default, Clone)]
pub struct PlatformSpecificWindowBuilderAttributes;

#[derive(Default, Clone, Debug)]
pub struct OsError;

impl Display for OsError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Android OS Error")
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
    pub fn size(&self) -> PhysicalSize {
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
