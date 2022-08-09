#![cfg(target_os = "android")]

use std::{
    collections::VecDeque,
    sync::{mpsc, RwLock},
    time::{Duration, Instant},
};

use ndk::{
    configuration::Configuration,
    event::{InputEvent, KeyAction, Keycode, MotionAction},
    looper::{ForeignLooper, Poll, ThreadLooper},
    native_window::NativeWindow,
};
use ndk_glue::{Event, LockReadGuard, Rect};
use once_cell::sync::Lazy;
use raw_window_handle::{
    AndroidDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error,
    event::{self, VirtualKeyCode},
    event_loop::{self, ControlFlow},
    monitor,
    window::{self, CursorGrabMode},
};

static CONFIG: Lazy<RwLock<Configuration>> = Lazy::new(|| {
    RwLock::new(Configuration::from_asset_manager(
        #[allow(deprecated)] // TODO: rust-windowing/winit#2196
        &ndk_glue::native_activity().asset_manager(),
    ))
});
// If this is `Some()` a `Poll::Wake` is considered an `EventSource::Internal` with the event
// contained in the `Option`. The event is moved outside of the `Option` replacing it with a
// `None`.
//
// This allows us to inject event into the event loop without going through `ndk-glue` and
// calling unsafe function that should only be called by Android.
static INTERNAL_EVENT: Lazy<RwLock<Option<InternalEvent>>> = Lazy::new(|| RwLock::new(None));

enum InternalEvent {
    RedrawRequested,
}

enum EventSource {
    Callback,
    InputQueue,
    User,
    Internal(InternalEvent),
}

fn ndk_keycode_to_virtualkeycode(keycode: Keycode) -> Option<event::VirtualKeyCode> {
    match keycode {
        Keycode::A => Some(VirtualKeyCode::A),
        Keycode::B => Some(VirtualKeyCode::B),
        Keycode::C => Some(VirtualKeyCode::C),
        Keycode::D => Some(VirtualKeyCode::D),
        Keycode::E => Some(VirtualKeyCode::E),
        Keycode::F => Some(VirtualKeyCode::F),
        Keycode::G => Some(VirtualKeyCode::G),
        Keycode::H => Some(VirtualKeyCode::H),
        Keycode::I => Some(VirtualKeyCode::I),
        Keycode::J => Some(VirtualKeyCode::J),
        Keycode::K => Some(VirtualKeyCode::K),
        Keycode::L => Some(VirtualKeyCode::L),
        Keycode::M => Some(VirtualKeyCode::M),
        Keycode::N => Some(VirtualKeyCode::N),
        Keycode::O => Some(VirtualKeyCode::O),
        Keycode::P => Some(VirtualKeyCode::P),
        Keycode::Q => Some(VirtualKeyCode::Q),
        Keycode::R => Some(VirtualKeyCode::R),
        Keycode::S => Some(VirtualKeyCode::S),
        Keycode::T => Some(VirtualKeyCode::T),
        Keycode::U => Some(VirtualKeyCode::U),
        Keycode::V => Some(VirtualKeyCode::V),
        Keycode::W => Some(VirtualKeyCode::W),
        Keycode::X => Some(VirtualKeyCode::X),
        Keycode::Y => Some(VirtualKeyCode::Y),
        Keycode::Z => Some(VirtualKeyCode::Z),

        Keycode::Keycode0 => Some(VirtualKeyCode::Key0),
        Keycode::Keycode1 => Some(VirtualKeyCode::Key1),
        Keycode::Keycode2 => Some(VirtualKeyCode::Key2),
        Keycode::Keycode3 => Some(VirtualKeyCode::Key3),
        Keycode::Keycode4 => Some(VirtualKeyCode::Key4),
        Keycode::Keycode5 => Some(VirtualKeyCode::Key5),
        Keycode::Keycode6 => Some(VirtualKeyCode::Key6),
        Keycode::Keycode7 => Some(VirtualKeyCode::Key7),
        Keycode::Keycode8 => Some(VirtualKeyCode::Key8),
        Keycode::Keycode9 => Some(VirtualKeyCode::Key9),

        Keycode::Numpad0 => Some(VirtualKeyCode::Numpad0),
        Keycode::Numpad1 => Some(VirtualKeyCode::Numpad1),
        Keycode::Numpad2 => Some(VirtualKeyCode::Numpad2),
        Keycode::Numpad3 => Some(VirtualKeyCode::Numpad3),
        Keycode::Numpad4 => Some(VirtualKeyCode::Numpad4),
        Keycode::Numpad5 => Some(VirtualKeyCode::Numpad5),
        Keycode::Numpad6 => Some(VirtualKeyCode::Numpad6),
        Keycode::Numpad7 => Some(VirtualKeyCode::Numpad7),
        Keycode::Numpad8 => Some(VirtualKeyCode::Numpad8),
        Keycode::Numpad9 => Some(VirtualKeyCode::Numpad9),

        Keycode::NumpadAdd => Some(VirtualKeyCode::NumpadAdd),
        Keycode::NumpadSubtract => Some(VirtualKeyCode::NumpadSubtract),
        Keycode::NumpadMultiply => Some(VirtualKeyCode::NumpadMultiply),
        Keycode::NumpadDivide => Some(VirtualKeyCode::NumpadDivide),
        Keycode::NumpadEnter => Some(VirtualKeyCode::NumpadEnter),
        Keycode::NumpadEquals => Some(VirtualKeyCode::NumpadEquals),
        Keycode::NumpadComma => Some(VirtualKeyCode::NumpadComma),
        Keycode::NumpadDot => Some(VirtualKeyCode::NumpadDecimal),
        Keycode::NumLock => Some(VirtualKeyCode::Numlock),

        Keycode::DpadLeft => Some(VirtualKeyCode::Left),
        Keycode::DpadRight => Some(VirtualKeyCode::Right),
        Keycode::DpadUp => Some(VirtualKeyCode::Up),
        Keycode::DpadDown => Some(VirtualKeyCode::Down),

        Keycode::F1 => Some(VirtualKeyCode::F1),
        Keycode::F2 => Some(VirtualKeyCode::F2),
        Keycode::F3 => Some(VirtualKeyCode::F3),
        Keycode::F4 => Some(VirtualKeyCode::F4),
        Keycode::F5 => Some(VirtualKeyCode::F5),
        Keycode::F6 => Some(VirtualKeyCode::F6),
        Keycode::F7 => Some(VirtualKeyCode::F7),
        Keycode::F8 => Some(VirtualKeyCode::F8),
        Keycode::F9 => Some(VirtualKeyCode::F9),
        Keycode::F10 => Some(VirtualKeyCode::F10),
        Keycode::F11 => Some(VirtualKeyCode::F11),
        Keycode::F12 => Some(VirtualKeyCode::F12),

        Keycode::Space => Some(VirtualKeyCode::Space),
        Keycode::Escape => Some(VirtualKeyCode::Escape),
        Keycode::Enter => Some(VirtualKeyCode::Return), // not on the Numpad
        Keycode::Tab => Some(VirtualKeyCode::Tab),

        Keycode::PageUp => Some(VirtualKeyCode::PageUp),
        Keycode::PageDown => Some(VirtualKeyCode::PageDown),
        Keycode::MoveHome => Some(VirtualKeyCode::Home),
        Keycode::MoveEnd => Some(VirtualKeyCode::End),
        Keycode::Insert => Some(VirtualKeyCode::Insert),

        Keycode::Del => Some(VirtualKeyCode::Back), // Backspace (above Enter)
        Keycode::ForwardDel => Some(VirtualKeyCode::Delete), // Delete (below Insert)

        Keycode::Copy => Some(VirtualKeyCode::Copy),
        Keycode::Paste => Some(VirtualKeyCode::Paste),
        Keycode::Cut => Some(VirtualKeyCode::Cut),

        Keycode::VolumeUp => Some(VirtualKeyCode::VolumeUp),
        Keycode::VolumeDown => Some(VirtualKeyCode::VolumeDown),
        Keycode::VolumeMute => Some(VirtualKeyCode::Mute), // ???
        Keycode::Mute => Some(VirtualKeyCode::Mute),       // ???
        Keycode::MediaPlayPause => Some(VirtualKeyCode::PlayPause),
        Keycode::MediaStop => Some(VirtualKeyCode::MediaStop), // ??? simple "Stop"?
        Keycode::MediaNext => Some(VirtualKeyCode::NextTrack),
        Keycode::MediaPrevious => Some(VirtualKeyCode::PrevTrack),

        Keycode::Plus => Some(VirtualKeyCode::Plus),
        Keycode::Minus => Some(VirtualKeyCode::Minus),
        Keycode::Equals => Some(VirtualKeyCode::Equals),
        Keycode::Semicolon => Some(VirtualKeyCode::Semicolon),
        Keycode::Slash => Some(VirtualKeyCode::Slash),
        Keycode::Backslash => Some(VirtualKeyCode::Backslash),
        Keycode::Comma => Some(VirtualKeyCode::Comma),
        Keycode::Period => Some(VirtualKeyCode::Period),
        Keycode::Apostrophe => Some(VirtualKeyCode::Apostrophe),
        Keycode::Grave => Some(VirtualKeyCode::Grave),
        Keycode::At => Some(VirtualKeyCode::At),

        // TODO: Maybe mapping this to Snapshot makes more sense? See: "PrtScr/SysRq"
        Keycode::Sysrq => Some(VirtualKeyCode::Sysrq),
        // These are usually the same (Pause/Break)
        Keycode::Break => Some(VirtualKeyCode::Pause),
        // These are exactly the same
        Keycode::ScrollLock => Some(VirtualKeyCode::Scroll),

        Keycode::Yen => Some(VirtualKeyCode::Yen),
        Keycode::Kana => Some(VirtualKeyCode::Kana),

        Keycode::CtrlLeft => Some(VirtualKeyCode::LControl),
        Keycode::CtrlRight => Some(VirtualKeyCode::RControl),

        Keycode::ShiftLeft => Some(VirtualKeyCode::LShift),
        Keycode::ShiftRight => Some(VirtualKeyCode::RShift),

        Keycode::AltLeft => Some(VirtualKeyCode::LAlt),
        Keycode::AltRight => Some(VirtualKeyCode::RAlt),

        // Different names for the same keys
        Keycode::MetaLeft => Some(VirtualKeyCode::LWin),
        Keycode::MetaRight => Some(VirtualKeyCode::RWin),

        Keycode::LeftBracket => Some(VirtualKeyCode::LBracket),
        Keycode::RightBracket => Some(VirtualKeyCode::RBracket),

        Keycode::Power => Some(VirtualKeyCode::Power),
        Keycode::Sleep => Some(VirtualKeyCode::Sleep), // what about SoftSleep?
        Keycode::Wakeup => Some(VirtualKeyCode::Wake),

        Keycode::NavigateNext => Some(VirtualKeyCode::NavigateForward),
        Keycode::NavigatePrevious => Some(VirtualKeyCode::NavigateBackward),

        Keycode::Calculator => Some(VirtualKeyCode::Calculator),
        Keycode::Explorer => Some(VirtualKeyCode::MyComputer), // "close enough"
        Keycode::Envelope => Some(VirtualKeyCode::Mail),       // "close enough"

        Keycode::Star => Some(VirtualKeyCode::Asterisk), // ???
        Keycode::AllApps => Some(VirtualKeyCode::Apps),  // ???
        Keycode::AppSwitch => Some(VirtualKeyCode::Apps), // ???
        Keycode::Refresh => Some(VirtualKeyCode::WebRefresh), // ???

        _ => None,
    }
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
    user_events_sender: mpsc::Sender<T>,
    user_events_receiver: mpsc::Receiver<T>,
    first_event: Option<EventSource>,
    start_cause: event::StartCause,
    looper: ThreadLooper,
    running: bool,
    window_lock: Option<LockReadGuard<NativeWindow>>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

macro_rules! call_event_handler {
    ( $event_handler:expr, $window_target:expr, $cf:expr, $event:expr ) => {{
        if let ControlFlow::ExitWithCode(code) = $cf {
            $event_handler($event, $window_target, &mut ControlFlow::ExitWithCode(code));
        } else {
            $event_handler($event, $window_target, &mut $cf);
        }
    }};
}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> Self {
        let (user_events_sender, user_events_receiver) = mpsc::channel();
        Self {
            window_target: event_loop::EventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    _marker: std::marker::PhantomData,
                },
                _marker: std::marker::PhantomData,
            },
            user_events_sender,
            user_events_receiver,
            first_event: None,
            start_cause: event::StartCause::Init,
            looper: ThreadLooper::for_thread().unwrap(),
            running: false,
            window_lock: None,
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

    pub fn run_return<F>(&mut self, mut event_handler: F) -> i32
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
                        // Acquire a lock on the window to prevent Android from destroying
                        // it until we've notified and waited for the user in Event::Suspended.
                        // WARNING: ndk-glue is inherently racy (https://github.com/rust-windowing/winit/issues/2293)
                        // and may have already received onNativeWindowDestroyed while this thread hasn't yet processed
                        // the event, and would see a `None` lock+window in that case.
                        if let Some(next_window_lock) = ndk_glue::native_window() {
                            assert!(
                                self.window_lock.replace(next_window_lock).is_none(),
                                "Received `Event::WindowCreated` while we were already holding a lock"
                            );
                            call_event_handler!(
                                event_handler,
                                self.window_target(),
                                control_flow,
                                event::Event::Resumed
                            );
                        } else {
                            warn!("Received `Event::WindowCreated` while `ndk_glue::native_window()` provides no window");
                        }
                    }
                    Event::WindowResized => resized = true,
                    Event::WindowRedrawNeeded => redraw = true,
                    Event::WindowDestroyed => {
                        // Release the lock, allowing Android to clean up this surface
                        // WARNING: See above - if ndk-glue is racy, this event may be called
                        // without having a `self.window_lock` in place.
                        if self.window_lock.take().is_some() {
                            call_event_handler!(
                                event_handler,
                                self.window_target(),
                                control_flow,
                                event::Event::Suspended
                            );
                        } else {
                            warn!("Received `Event::WindowDestroyed` while we were not holding a window lock");
                        }
                    }
                    Event::Pause => self.running = false,
                    Event::Resume => self.running = true,
                    Event::ConfigChanged => {
                        #[allow(deprecated)] // TODO: rust-windowing/winit#2196
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
                        while let Some(event) = input_queue.get_event().expect("get_event") {
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
                                                    virtual_keycode: ndk_keycode_to_virtualkeycode(
                                                        key.key_code(),
                                                    ),
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
                    // try_recv only errors when empty (expected) or disconnect. But because Self
                    // contains a Sender it will never disconnect, so no error handling need.
                    while let Ok(event) = self.user_events_receiver.try_recv() {
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
                ControlFlow::ExitWithCode(code) => {
                    self.first_event = poll(
                        self.looper
                            .poll_once_timeout(Duration::from_millis(0))
                            .unwrap(),
                    );
                    self.start_cause = event::StartCause::WaitCancelled {
                        start: Instant::now(),
                        requested_resume: None,
                    };
                    break 'event_loop code;
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
            user_events_sender: self.user_events_sender.clone(),
            looper: ForeignLooper::for_thread().expect("called from event loop thread"),
        }
    }
}

pub struct EventLoopProxy<T: 'static> {
    user_events_sender: mpsc::Sender<T>,
    looper: ForeignLooper,
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), event_loop::EventLoopClosed<T>> {
        self.user_events_sender
            .send(event)
            .map_err(|mpsc::SendError(x)| event_loop::EventLoopClosed(x))?;
        self.looper.wake();
        Ok(())
    }
}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy {
            user_events_sender: self.user_events_sender.clone(),
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

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Android(AndroidDisplayHandle::empty())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WindowId;

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

pub struct Window;

impl Window {
    pub(crate) fn new<T: 'static>(
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

    pub fn is_visible(&self) -> Option<bool> {
        None
    }

    pub fn set_resizable(&self, _resizeable: bool) {}

    pub fn is_resizable(&self) -> bool {
        false
    }

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

    pub fn is_decorated(&self) -> bool {
        true
    }

    pub fn set_always_on_top(&self, _always_on_top: bool) {}

    pub fn set_window_icon(&self, _window_icon: Option<crate::icon::Icon>) {}

    pub fn set_ime_position(&self, _position: Position) {}

    pub fn set_ime_allowed(&self, _allowed: bool) {}

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

    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        if let Some(native_window) = ndk_glue::native_window() {
            native_window.raw_window_handle()
        } else {
            panic!("Cannot get the native window, it's null and will always be null before Event::Resumed and after Event::Suspended. Make sure you only call this function between those events.");
        }
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Android(AndroidDisplayHandle::empty())
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

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        // FIXME no way to get real refresh rate for now.
        None
    }

    pub fn video_modes(&self) -> impl Iterator<Item = monitor::VideoMode> {
        let size = self.size().into();
        // FIXME this is not the real refresh rate
        // (it is guaranteed to support 32 bit color though)
        std::iter::once(monitor::VideoMode {
            video_mode: VideoMode {
                size,
                bit_depth: 32,
                refresh_rate_millihertz: 60000,
                monitor: self.clone(),
            },
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

    pub fn monitor(&self) -> monitor::MonitorHandle {
        monitor::MonitorHandle {
            inner: self.monitor.clone(),
        }
    }
}
