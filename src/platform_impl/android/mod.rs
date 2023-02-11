#![cfg(android_platform)]

use std::{
    collections::VecDeque,
    hash::Hash,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, RwLock,
    },
    time::{Duration, Instant},
};

use android_activity::input::{InputEvent, KeyAction, Keycode, MotionAction};
use android_activity::{
    AndroidApp, AndroidAppWaker, ConfigurationRef, InputStatus, MainEvent, Rect,
};
use once_cell::sync::Lazy;
use raw_window_handle::{
    AndroidDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::platform_impl::Fullscreen;
use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error,
    event::{self, StartCause, VirtualKeyCode},
    event_loop::{self, ControlFlow, EventLoopWindowTarget as RootELW},
    window::{
        self, CursorGrabMode, ImePurpose, ResizeDirection, Theme, WindowButtons, WindowLevel,
    },
};

static HAS_FOCUS: Lazy<RwLock<bool>> = Lazy::new(|| RwLock::new(true));

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
                    let device_id = event::DeviceId(DeviceId);

                    let state = match key.action() {
                        KeyAction::Down => event::ElementState::Pressed,
                        KeyAction::Up => event::ElementState::Released,
                        _ => event::ElementState::Released,
                    };
                    #[allow(deprecated)]
                    let event = event::Event::WindowEvent {
                        window_id: window::WindowId(WindowId),
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
                    sticky_exit_callback(
                        event,
                        self.window_target(),
                        control_flow,
                        callback
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
