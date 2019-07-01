#![cfg(target_os = "android")]

mod ffi;

use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use crate::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use crate::error;
use crate::event;
use crate::event_loop::{self, ControlFlow};
use crate::monitor;
use crate::window;

// TODO: move native event parsing to Winit
// Avoid android_glue::Event entirely, go AInputEvent* directly to winit::event::Event<T>
pub struct EventLoop<T: 'static> {
    window_target: event_loop::EventLoopWindowTarget<T>,
    rx: mpsc::Receiver<android_glue::Event>,
    user_queue: Arc<Mutex<VecDeque<T>>>,
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        android_glue::add_sender(tx.clone());
        Self {
            window_target: event_loop::EventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    tx,
                    _marker: std::marker::PhantomData,
                },
                _marker: std::marker::PhantomData,
            },
            rx,
            user_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    // Returns true if it received a `Destroy` event
    fn do_event<F>(
        &self,
        glue_event: android_glue::Event,
        mut event_handler: F,
        cf: &mut ControlFlow,
    ) where
        F: FnMut(event::Event<T>, &event_loop::EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        if let Some(event) = self.glue_event_to_event(glue_event) {
            event_handler(event, self.window_target(), cf);

            // Ugly because android_glue::Event doesn't impl Eq
            match glue_event {
                android_glue::Event::Destroy => self.gracefully_exit(),
                _ => (),
            }
        }
    }

    /// Used when the event loop is destroyed
    fn gracefully_exit(&self) -> ! {
        unsafe { ffi::pthread_exit() }
    }

    pub fn run<F>(self, mut event_handler: F) -> !
    where
        F: 'static
            + FnMut(event::Event<T>, &event_loop::EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let mut cf = ControlFlow::default();

        let mut start_cause = event::StartCause::Init;
        let mut first_event = None;

        loop {
            event_handler(
                event::Event::NewEvents(start_cause),
                self.window_target(),
                &mut cf,
            );
            if let Some(glue_event) = first_event.take() {
                self.do_event(glue_event, &mut event_handler, &mut cf);
            }
            for glue_event in self.rx.try_iter() {
                self.do_event(glue_event, &mut event_handler, &mut cf);
            }
            event_handler(event::Event::EventsCleared, self.window_target(), &mut cf);

            if cf == ControlFlow::Exit {
                // This should not happen...
                cf = ControlFlow::default();
            }

            match cf {
                ControlFlow::Exit => unreachable!(),
                ControlFlow::Poll => {
                    start_cause = event::StartCause::Poll;
                }
                ControlFlow::Wait => {
                    start_cause = event::StartCause::WaitCancelled {
                        start: Instant::now(),
                        requested_resume: None,
                    };
                    first_event = Some(loop {
                        let e = self.rx.recv().unwrap();
                        if self.glue_event_to_event(e).is_some() {
                            break e;
                        }
                    });
                }
                ControlFlow::WaitUntil(instant) => {
                    let start = Instant::now();
                    loop {
                        let now = Instant::now();
                        match self.rx.recv_timeout(instant - now) {
                            Ok(e) => {
                                first_event = Some(e);
                                start_cause = event::StartCause::WaitCancelled {
                                    start,
                                    requested_resume: Some(instant),
                                };
                                if self.glue_event_to_event(e).is_some() {
                                    break;
                                }
                            }
                            Err(_) => {
                                start_cause = event::StartCause::ResumeTimeReached {
                                    start,
                                    requested_resume: instant,
                                };
                                break;
                            }
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
        unsafe { MonitorHandle(android_glue::get_native_window() as *const _) }
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(self.primary_monitor());
        v
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            tx: self.window_target().p.tx.clone(),
            queue: self.user_queue.clone(),
        }
    }

    fn glue_event_to_event(&self, glue_event: android_glue::Event) -> Option<event::Event<T>> {
        let window_id = crate::window::WindowId(WindowId);
        let device_id = event::DeviceId(DeviceId);

        match glue_event {
            android_glue::Event::EventMotion(motion_event) => Some(event::Event::WindowEvent {
                window_id,
                event: event::WindowEvent::Touch(event::Touch {
                    device_id,
                    phase: match motion_event.action {
                        android_glue::MotionAction::Down => event::TouchPhase::Started,
                        android_glue::MotionAction::Up => event::TouchPhase::Ended,
                        android_glue::MotionAction::Move => event::TouchPhase::Moved,
                        android_glue::MotionAction::Cancel => event::TouchPhase::Cancelled,
                    },
                    location: crate::dpi::LogicalPosition::from_physical(
                        (motion_event.x as f64, motion_event.y as f64),
                        self.get_dpi_factor(),
                    ),
                    id: motion_event.pointer_id as u64,
                }),
            }),
            android_glue::Event::User => Some(event::Event::UserEvent(
                self.user_queue.lock().unwrap().pop_front().unwrap(),
            )),
            android_glue::Event::Start => None,
            android_glue::Event::Pause => None,
            android_glue::Event::Resume => None,
            android_glue::Event::Stop => None,
            android_glue::Event::Destroy => Some(event::Event::LoopDestroyed),
            android_glue::Event::SaveState => None, // TODO save the state now
            android_glue::Event::LostFocus => Some(event::Event::WindowEvent {
                window_id: window::WindowId(WindowId),
                event: event::WindowEvent::Focused(false),
            }),
            android_glue::Event::GainedFocus => Some(event::Event::WindowEvent {
                window_id: window::WindowId(WindowId),
                event: event::WindowEvent::Focused(true),
            }),
            android_glue::Event::ConfigChanged => None, // TODO maybe notify HiDPI changed and/or resized
            _ => None,
        }
    }

    // FIXME this is not true
    fn get_dpi_factor(&self) -> f64 {
        1.
    }
}

#[derive(Clone)]
pub struct EventLoopProxy<T: 'static> {
    tx: mpsc::Sender<android_glue::Event>,
    queue: Arc<Mutex<VecDeque<T>>>,
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), event_loop::EventLoopClosed> {
        self.queue.lock().unwrap().push_back(event);
        self.tx
            .send(android_glue::Event::User)
            .map_err(|_| event_loop::EventLoopClosed)
    }
}

pub struct EventLoopWindowTarget<T: 'static> {
    tx: mpsc::Sender<android_glue::Event>,
    _marker: std::marker::PhantomData<T>,
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

#[derive(Debug, Clone)]
pub struct MonitorHandle(*const ffi::ANativeWindow);

impl MonitorHandle {
    pub fn name(&self) -> Option<String> {
        Some("Android Device".to_owned())
    }

    pub fn size(&self) -> PhysicalSize {
        // FIXME this is the window size, not the monitor size
        unsafe {
            let width = ffi::ANativeWindow_getWidth(self.0) as f64;
            let height = ffi::ANativeWindow_getHeight(self.0) as f64;
            PhysicalSize::new(width, height)
        }
    }

    pub fn position(&self) -> PhysicalPosition {
        (0, 0).into()
    }

    pub fn hidpi_factor(&self) -> f64 {
        // TODO legit hidpi factors
        1.0
    }

    pub fn video_modes(&self) -> impl Iterator<Item = monitor::VideoMode> {
        let size = self.size().into();
        let mut v = Vec::new();
        // FIXME this is not the real refresh rate
        // (it is guarunteed to support 32 bit color though)
        v.push(monitor::VideoMode {
            size,
            bit_depth: 32,
            refresh_rate: 60,
        });
        v.into_iter()
    }
}

pub struct Window {
    native_window: *mut ffi::ANativeWindow,
    tx: mpsc::Sender<android_glue::Event>,
}

impl Window {
    pub fn new<T: 'static>(
        el: &EventLoopWindowTarget<T>,
        _window_attrs: window::WindowAttributes,
        _: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, error::OsError> {
        // FIXME this ignores requested window attributes
        Ok(Self {
            native_window: unsafe { android_glue::get_native_window() as *mut _ },
            tx: el.tx.clone(),
        })
    }

    pub fn id(&self) -> WindowId {
        WindowId
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle(self.native_window)
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle(self.native_window));
        v
    }

    pub fn current_monitor(&self) -> monitor::MonitorHandle {
        monitor::MonitorHandle {
            inner: MonitorHandle(self.native_window),
        }
    }

    pub fn hidpi_factor(&self) -> f64 {
        // TODO legit hidpi factors
        1.0
    }

    pub fn request_redraw(&self) {
        self.tx
            .send(android_glue::Event::WindowRedrawNeeded)
            .unwrap();
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
        unsafe {
            let width = ffi::ANativeWindow_getWidth(self.native_window) as f64;
            let height = ffi::ANativeWindow_getHeight(self.native_window) as f64;
            LogicalSize::from_physical((width, height), self.hidpi_factor())
        }
    }

    pub fn set_inner_size(&self, _size: LogicalSize) {
        panic!("Cannot set window size on Android");
    }

    pub fn outer_size(&self) -> LogicalSize {
        // FIXME wrong
        self.inner_size()
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

    pub fn set_maximized(&self, _maximized: bool) {
        // no effect
    }

    pub fn set_fullscreen(&self, _monitor: Option<monitor::MonitorHandle>) {
        // no effect
    }

    pub fn fullscreen(&self) -> Option<monitor::MonitorHandle> {
        Some(monitor::MonitorHandle {
            inner: MonitorHandle(self.native_window),
        })
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
