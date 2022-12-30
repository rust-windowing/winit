use orbclient::{
    ButtonEvent, EventOption, FocusEvent, HoverEvent, KeyEvent, MouseEvent, MoveEvent, QuitEvent,
    ResizeEvent, ScrollEvent, TextInputEvent,
};
use raw_window_handle::{OrbitalDisplayHandle, RawDisplayHandle};
use std::{
    collections::VecDeque,
    mem, slice,
    sync::{mpsc, Arc, Mutex},
    time::Instant,
};

use crate::{
    event::{self, StartCause, VirtualKeyCode},
    event_loop::{self, ControlFlow},
    window::WindowId as RootWindowId,
};

use super::{
    DeviceId, MonitorHandle, PlatformSpecificEventLoopAttributes, RedoxSocket, TimeSocket,
    WindowId, WindowProperties,
};

fn convert_scancode(scancode: u8) -> Option<VirtualKeyCode> {
    match scancode {
        orbclient::K_A => Some(VirtualKeyCode::A),
        orbclient::K_B => Some(VirtualKeyCode::B),
        orbclient::K_C => Some(VirtualKeyCode::C),
        orbclient::K_D => Some(VirtualKeyCode::D),
        orbclient::K_E => Some(VirtualKeyCode::E),
        orbclient::K_F => Some(VirtualKeyCode::F),
        orbclient::K_G => Some(VirtualKeyCode::G),
        orbclient::K_H => Some(VirtualKeyCode::H),
        orbclient::K_I => Some(VirtualKeyCode::I),
        orbclient::K_J => Some(VirtualKeyCode::J),
        orbclient::K_K => Some(VirtualKeyCode::K),
        orbclient::K_L => Some(VirtualKeyCode::L),
        orbclient::K_M => Some(VirtualKeyCode::M),
        orbclient::K_N => Some(VirtualKeyCode::N),
        orbclient::K_O => Some(VirtualKeyCode::O),
        orbclient::K_P => Some(VirtualKeyCode::P),
        orbclient::K_Q => Some(VirtualKeyCode::Q),
        orbclient::K_R => Some(VirtualKeyCode::R),
        orbclient::K_S => Some(VirtualKeyCode::S),
        orbclient::K_T => Some(VirtualKeyCode::T),
        orbclient::K_U => Some(VirtualKeyCode::U),
        orbclient::K_V => Some(VirtualKeyCode::V),
        orbclient::K_W => Some(VirtualKeyCode::W),
        orbclient::K_X => Some(VirtualKeyCode::X),
        orbclient::K_Y => Some(VirtualKeyCode::Y),
        orbclient::K_Z => Some(VirtualKeyCode::Z),
        orbclient::K_0 => Some(VirtualKeyCode::Key0),
        orbclient::K_1 => Some(VirtualKeyCode::Key1),
        orbclient::K_2 => Some(VirtualKeyCode::Key2),
        orbclient::K_3 => Some(VirtualKeyCode::Key3),
        orbclient::K_4 => Some(VirtualKeyCode::Key4),
        orbclient::K_5 => Some(VirtualKeyCode::Key5),
        orbclient::K_6 => Some(VirtualKeyCode::Key6),
        orbclient::K_7 => Some(VirtualKeyCode::Key7),
        orbclient::K_8 => Some(VirtualKeyCode::Key8),
        orbclient::K_9 => Some(VirtualKeyCode::Key9),

        orbclient::K_TICK => Some(VirtualKeyCode::Grave),
        orbclient::K_MINUS => Some(VirtualKeyCode::Minus),
        orbclient::K_EQUALS => Some(VirtualKeyCode::Equals),
        orbclient::K_BACKSLASH => Some(VirtualKeyCode::Backslash),
        orbclient::K_BRACE_OPEN => Some(VirtualKeyCode::LBracket),
        orbclient::K_BRACE_CLOSE => Some(VirtualKeyCode::RBracket),
        orbclient::K_SEMICOLON => Some(VirtualKeyCode::Semicolon),
        orbclient::K_QUOTE => Some(VirtualKeyCode::Apostrophe),
        orbclient::K_COMMA => Some(VirtualKeyCode::Comma),
        orbclient::K_PERIOD => Some(VirtualKeyCode::Period),
        orbclient::K_SLASH => Some(VirtualKeyCode::Slash),
        orbclient::K_BKSP => Some(VirtualKeyCode::Back),
        orbclient::K_SPACE => Some(VirtualKeyCode::Space),
        orbclient::K_TAB => Some(VirtualKeyCode::Tab),
        //orbclient::K_CAPS => Some(VirtualKeyCode::CAPS),
        orbclient::K_LEFT_SHIFT => Some(VirtualKeyCode::LShift),
        orbclient::K_RIGHT_SHIFT => Some(VirtualKeyCode::RShift),
        orbclient::K_CTRL => Some(VirtualKeyCode::LControl),
        orbclient::K_ALT => Some(VirtualKeyCode::LAlt),
        orbclient::K_ENTER => Some(VirtualKeyCode::Return),
        orbclient::K_ESC => Some(VirtualKeyCode::Escape),
        orbclient::K_F1 => Some(VirtualKeyCode::F1),
        orbclient::K_F2 => Some(VirtualKeyCode::F2),
        orbclient::K_F3 => Some(VirtualKeyCode::F3),
        orbclient::K_F4 => Some(VirtualKeyCode::F4),
        orbclient::K_F5 => Some(VirtualKeyCode::F5),
        orbclient::K_F6 => Some(VirtualKeyCode::F6),
        orbclient::K_F7 => Some(VirtualKeyCode::F7),
        orbclient::K_F8 => Some(VirtualKeyCode::F8),
        orbclient::K_F9 => Some(VirtualKeyCode::F9),
        orbclient::K_F10 => Some(VirtualKeyCode::F10),
        orbclient::K_HOME => Some(VirtualKeyCode::Home),
        orbclient::K_UP => Some(VirtualKeyCode::Up),
        orbclient::K_PGUP => Some(VirtualKeyCode::PageUp),
        orbclient::K_LEFT => Some(VirtualKeyCode::Left),
        orbclient::K_RIGHT => Some(VirtualKeyCode::Right),
        orbclient::K_END => Some(VirtualKeyCode::End),
        orbclient::K_DOWN => Some(VirtualKeyCode::Down),
        orbclient::K_PGDN => Some(VirtualKeyCode::PageDown),
        orbclient::K_DEL => Some(VirtualKeyCode::Delete),
        orbclient::K_F11 => Some(VirtualKeyCode::F11),
        orbclient::K_F12 => Some(VirtualKeyCode::F12),

        _ => None,
    }
}

fn element_state(pressed: bool) -> event::ElementState {
    if pressed {
        event::ElementState::Pressed
    } else {
        event::ElementState::Released
    }
}

#[derive(Default)]
struct EventState {
    lshift: bool,
    rshift: bool,
    lctrl: bool,
    rctrl: bool,
    lalt: bool,
    ralt: bool,
    llogo: bool,
    rlogo: bool,
    left: bool,
    middle: bool,
    right: bool,
}

impl EventState {
    fn key(&mut self, vk: VirtualKeyCode, pressed: bool) {
        match vk {
            VirtualKeyCode::LShift => self.lshift = pressed,
            VirtualKeyCode::RShift => self.rshift = pressed,
            VirtualKeyCode::LControl => self.lctrl = pressed,
            VirtualKeyCode::RControl => self.rctrl = pressed,
            VirtualKeyCode::LAlt => self.lalt = pressed,
            VirtualKeyCode::RAlt => self.ralt = pressed,
            VirtualKeyCode::LWin => self.llogo = pressed,
            VirtualKeyCode::RWin => self.rlogo = pressed,
            _ => (),
        }
    }

    fn mouse(
        &mut self,
        left: bool,
        middle: bool,
        right: bool,
    ) -> Option<(event::MouseButton, event::ElementState)> {
        if self.left != left {
            self.left = left;
            return Some((event::MouseButton::Left, element_state(self.left)));
        }

        if self.middle != middle {
            self.middle = middle;
            return Some((event::MouseButton::Middle, element_state(self.middle)));
        }

        if self.right != right {
            self.right = right;
            return Some((event::MouseButton::Right, element_state(self.right)));
        }

        None
    }

    fn modifiers(&self) -> event::ModifiersState {
        let mut modifiers = event::ModifiersState::empty();
        if self.lshift || self.rshift {
            modifiers |= event::ModifiersState::SHIFT;
        }
        if self.lctrl || self.rctrl {
            modifiers |= event::ModifiersState::CTRL;
        }
        if self.lalt || self.ralt {
            modifiers |= event::ModifiersState::ALT;
        }
        if self.llogo || self.rlogo {
            modifiers |= event::ModifiersState::LOGO
        }
        modifiers
    }
}

pub struct EventLoop<T: 'static> {
    windows: Vec<Arc<RedoxSocket>>,
    window_target: event_loop::EventLoopWindowTarget<T>,
    state: EventState,
}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> Self {
        let (user_events_sender, user_events_receiver) = mpsc::channel();

        let event_socket = Arc::new(RedoxSocket::event().unwrap());

        let wake_socket = Arc::new(TimeSocket::open().unwrap());

        event_socket
            .write(&syscall::Event {
                id: wake_socket.0.fd,
                flags: syscall::EventFlags::EVENT_READ,
                data: wake_socket.0.fd,
            })
            .unwrap();

        Self {
            windows: Vec::new(),
            window_target: event_loop::EventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    user_events_sender,
                    user_events_receiver,
                    creates: Mutex::new(VecDeque::new()),
                    redraws: Arc::new(Mutex::new(VecDeque::new())),
                    destroys: Arc::new(Mutex::new(VecDeque::new())),
                    event_socket,
                    wake_socket,
                },
                _marker: std::marker::PhantomData,
            },
            state: EventState::default(),
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

    pub fn run_return<F>(&mut self, mut event_handler_inner: F) -> i32
    where
        F: FnMut(event::Event<'_, T>, &event_loop::EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        // Wrapper for event handler function that prevents ExitWithCode from being unset.
        let mut event_handler =
            move |event: event::Event<'_, T>,
                  window_target: &event_loop::EventLoopWindowTarget<T>,
                  control_flow: &mut ControlFlow| {
                if let ControlFlow::ExitWithCode(code) = control_flow {
                    event_handler_inner(
                        event,
                        window_target,
                        &mut ControlFlow::ExitWithCode(*code),
                    );
                } else {
                    event_handler_inner(event, window_target, control_flow);
                }
            };

        let mut control_flow = ControlFlow::default();
        let mut start_cause = StartCause::Init;

        let code = loop {
            event_handler(
                event::Event::NewEvents(start_cause),
                &self.window_target,
                &mut control_flow,
            );

            if start_cause == StartCause::Init {
                event_handler(
                    event::Event::Resumed,
                    &self.window_target,
                    &mut control_flow,
                );
            }

            // Handle window creates.
            while let Some(window) = {
                let mut creates = self.window_target.p.creates.lock().unwrap();
                creates.pop_front()
            } {
                let window_id = WindowId {
                    fd: window.fd as u64,
                };

                let mut buf: [u8; 4096] = [0; 4096];
                let path = window.fpath(&mut buf).expect("failed to read properties");
                let properties = WindowProperties::new(path);

                self.windows.push(window);

                // Send resize event on create to indicate first size.
                event_handler(
                    event::Event::WindowEvent {
                        window_id: RootWindowId(window_id),
                        event: event::WindowEvent::Resized((properties.w, properties.h).into()),
                    },
                    &self.window_target,
                    &mut control_flow,
                );

                // Send resize event on create to indicate first position.
                event_handler(
                    event::Event::WindowEvent {
                        window_id: RootWindowId(window_id),
                        event: event::WindowEvent::Moved((properties.x, properties.y).into()),
                    },
                    &self.window_target,
                    &mut control_flow,
                );
            }

            // Handle window destroys.
            while let Some(destroy_id) = {
                let mut destroys = self.window_target.p.destroys.lock().unwrap();
                destroys.pop_front()
            } {
                event_handler(
                    event::Event::WindowEvent {
                        window_id: RootWindowId(destroy_id),
                        event: event::WindowEvent::Destroyed,
                    },
                    &self.window_target,
                    &mut control_flow,
                );

                self.windows
                    .retain(|window| window.fd as u64 != destroy_id.fd);
            }

            // Handle window events.
            let mut i = 0;
            let mut resize_opt = None;
            // While loop is used here because the same window may be processed more than once.
            while let Some(window) = self.windows.get(i) {
                let window_id = WindowId {
                    fd: window.fd as u64,
                };

                let mut event_buf = [0u8; 16 * mem::size_of::<orbclient::Event>()];
                let count =
                    syscall::read(window.fd, &mut event_buf).expect("failed to read window events");
                // Safety: orbclient::Event is a packed struct designed to be transferred over a socket.
                let events = unsafe {
                    slice::from_raw_parts(
                        event_buf.as_ptr() as *const orbclient::Event,
                        count / mem::size_of::<orbclient::Event>(),
                    )
                };

                for event in events {
                    match event.to_option() {
                        EventOption::Key(KeyEvent {
                            character: _,
                            scancode,
                            pressed,
                        }) => {
                            if scancode != 0 {
                                let vk_opt = convert_scancode(scancode);
                                if let Some(vk) = vk_opt {
                                    self.state.key(vk, pressed);
                                }
                                event_handler(
                                    #[allow(deprecated)]
                                    event::Event::WindowEvent {
                                        window_id: RootWindowId(window_id),
                                        event: event::WindowEvent::KeyboardInput {
                                            device_id: event::DeviceId(DeviceId),
                                            input: event::KeyboardInput {
                                                scancode: scancode as u32,
                                                state: element_state(pressed),
                                                virtual_keycode: vk_opt,
                                                modifiers: self.state.modifiers(),
                                            },
                                            is_synthetic: false,
                                        },
                                    },
                                    &self.window_target,
                                    &mut control_flow,
                                );
                            }
                        }
                        EventOption::TextInput(TextInputEvent { character }) => {
                            event_handler(
                                event::Event::WindowEvent {
                                    window_id: RootWindowId(window_id),
                                    event: event::WindowEvent::ReceivedCharacter(character),
                                },
                                &self.window_target,
                                &mut control_flow,
                            );
                        }
                        EventOption::Mouse(MouseEvent { x, y }) => {
                            event_handler(
                                event::Event::WindowEvent {
                                    window_id: RootWindowId(window_id),
                                    event: event::WindowEvent::CursorMoved {
                                        device_id: event::DeviceId(DeviceId),
                                        position: (x, y).into(),
                                        modifiers: self.state.modifiers(),
                                    },
                                },
                                &self.window_target,
                                &mut control_flow,
                            );
                        }
                        EventOption::Button(ButtonEvent {
                            left,
                            middle,
                            right,
                        }) => {
                            while let Some((button, state)) = self.state.mouse(left, middle, right)
                            {
                                event_handler(
                                    event::Event::WindowEvent {
                                        window_id: RootWindowId(window_id),
                                        event: event::WindowEvent::MouseInput {
                                            device_id: event::DeviceId(DeviceId),
                                            state,
                                            button,
                                            modifiers: self.state.modifiers(),
                                        },
                                    },
                                    &self.window_target,
                                    &mut control_flow,
                                );
                            }
                        }
                        EventOption::Scroll(ScrollEvent { x, y }) => {
                            event_handler(
                                event::Event::WindowEvent {
                                    window_id: RootWindowId(window_id),
                                    event: event::WindowEvent::MouseWheel {
                                        device_id: event::DeviceId(DeviceId),
                                        delta: event::MouseScrollDelta::LineDelta(
                                            x as f32, y as f32,
                                        ),
                                        phase: event::TouchPhase::Moved,
                                        modifiers: self.state.modifiers(),
                                    },
                                },
                                &self.window_target,
                                &mut control_flow,
                            );
                        }
                        EventOption::Quit(QuitEvent {}) => {
                            event_handler(
                                event::Event::WindowEvent {
                                    window_id: RootWindowId(window_id),
                                    event: event::WindowEvent::CloseRequested,
                                },
                                &self.window_target,
                                &mut control_flow,
                            );
                        }
                        EventOption::Focus(FocusEvent { focused }) => {
                            event_handler(
                                event::Event::WindowEvent {
                                    window_id: RootWindowId(window_id),
                                    event: event::WindowEvent::Focused(focused),
                                },
                                &self.window_target,
                                &mut control_flow,
                            );
                        }
                        EventOption::Move(MoveEvent { x, y }) => {
                            event_handler(
                                event::Event::WindowEvent {
                                    window_id: RootWindowId(window_id),
                                    event: event::WindowEvent::Moved((x, y).into()),
                                },
                                &self.window_target,
                                &mut control_flow,
                            );
                        }
                        EventOption::Resize(ResizeEvent { width, height }) => {
                            event_handler(
                                event::Event::WindowEvent {
                                    window_id: RootWindowId(window_id),
                                    event: event::WindowEvent::Resized((width, height).into()),
                                },
                                &self.window_target,
                                &mut control_flow,
                            );

                            // Acknowledge resize after event loop.
                            resize_opt = Some((width, height));
                        }
                        //TODO: Clipboard
                        EventOption::Hover(HoverEvent { entered }) => {
                            if entered {
                                event_handler(
                                    event::Event::WindowEvent {
                                        window_id: RootWindowId(window_id),
                                        event: event::WindowEvent::CursorEntered {
                                            device_id: event::DeviceId(DeviceId),
                                        },
                                    },
                                    &self.window_target,
                                    &mut control_flow,
                                );
                            } else {
                                event_handler(
                                    event::Event::WindowEvent {
                                        window_id: RootWindowId(window_id),
                                        event: event::WindowEvent::CursorLeft {
                                            device_id: event::DeviceId(DeviceId),
                                        },
                                    },
                                    &self.window_target,
                                    &mut control_flow,
                                );
                            }
                        }
                        other => {
                            warn!("unhandled event: {:?}", other);
                        }
                    }
                }

                if count == event_buf.len() {
                    // If event buf was full, process same window again to ensure all events are drained.
                    continue;
                }

                // Acknowledge the latest resize event.
                if let Some((w, h)) = resize_opt.take() {
                    window
                        .write(format!("S,{},{}", w, h).as_bytes())
                        .expect("failed to acknowledge resize");

                    // Require redraw after resize.
                    let mut redraws = self.window_target.p.redraws.lock().unwrap();
                    if !redraws.contains(&window_id) {
                        redraws.push_back(window_id);
                    }
                }

                // Move to next window.
                i += 1;
            }

            while let Ok(event) = self.window_target.p.user_events_receiver.try_recv() {
                event_handler(
                    event::Event::UserEvent(event),
                    &self.window_target,
                    &mut control_flow,
                );
            }

            event_handler(
                event::Event::MainEventsCleared,
                &self.window_target,
                &mut control_flow,
            );

            // To avoid deadlocks the redraws lock is not held during event processing.
            while let Some(window_id) = {
                let mut redraws = self.window_target.p.redraws.lock().unwrap();
                redraws.pop_front()
            } {
                event_handler(
                    event::Event::RedrawRequested(RootWindowId(window_id)),
                    &self.window_target,
                    &mut control_flow,
                );
            }

            event_handler(
                event::Event::RedrawEventsCleared,
                &self.window_target,
                &mut control_flow,
            );

            let requested_resume = match control_flow {
                ControlFlow::Poll => {
                    start_cause = StartCause::Poll;
                    continue;
                }
                ControlFlow::Wait => None,
                ControlFlow::WaitUntil(instant) => Some(instant),
                ControlFlow::ExitWithCode(code) => break code,
            };

            // Re-using wake socket caused extra wake events before because there were leftover
            // timeouts, and then new timeouts were added each time a spurious timeout expired.
            let timeout_socket = TimeSocket::open().unwrap();

            self.window_target
                .p
                .event_socket
                .write(&syscall::Event {
                    id: timeout_socket.0.fd,
                    flags: syscall::EventFlags::EVENT_READ,
                    data: 0,
                })
                .unwrap();

            let start = Instant::now();
            if let Some(instant) = requested_resume {
                let mut time = timeout_socket.current_time().unwrap();

                if let Some(duration) = instant.checked_duration_since(start) {
                    time.tv_sec += duration.as_secs() as i64;
                    time.tv_nsec += duration.subsec_nanos() as i32;
                    // Normalize timespec so tv_nsec is not greater than one second.
                    while time.tv_nsec >= 1_000_000_000 {
                        time.tv_sec += 1;
                        time.tv_nsec -= 1_000_000_000;
                    }
                }

                timeout_socket.timeout(&time).unwrap();
            }

            // Wait for event if needed.
            let mut event = syscall::Event::default();
            self.window_target.p.event_socket.read(&mut event).unwrap();

            // TODO: handle spurious wakeups (redraw caused wakeup but redraw already handled)
            match requested_resume {
                Some(requested_resume) if event.id == timeout_socket.0.fd => {
                    // If the event is from the special timeout socket, report that resume
                    // time was reached.
                    start_cause = StartCause::ResumeTimeReached {
                        start,
                        requested_resume,
                    };
                }
                _ => {
                    // Normal window event or spurious timeout.
                    start_cause = StartCause::WaitCancelled {
                        start,
                        requested_resume,
                    };
                }
            }
        };

        event_handler(
            event::Event::LoopDestroyed,
            &self.window_target,
            &mut control_flow,
        );

        code
    }

    pub fn window_target(&self) -> &event_loop::EventLoopWindowTarget<T> {
        &self.window_target
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            user_events_sender: self.window_target.p.user_events_sender.clone(),
            wake_socket: self.window_target.p.wake_socket.clone(),
        }
    }
}

pub struct EventLoopProxy<T: 'static> {
    user_events_sender: mpsc::Sender<T>,
    wake_socket: Arc<TimeSocket>,
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), event_loop::EventLoopClosed<T>> {
        self.user_events_sender
            .send(event)
            .map_err(|mpsc::SendError(x)| event_loop::EventLoopClosed(x))?;

        self.wake_socket.wake().unwrap();

        Ok(())
    }
}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            user_events_sender: self.user_events_sender.clone(),
            wake_socket: self.wake_socket.clone(),
        }
    }
}

impl<T> Unpin for EventLoopProxy<T> {}

pub struct EventLoopWindowTarget<T: 'static> {
    pub(super) user_events_sender: mpsc::Sender<T>,
    pub(super) user_events_receiver: mpsc::Receiver<T>,
    pub(super) creates: Mutex<VecDeque<Arc<RedoxSocket>>>,
    pub(super) redraws: Arc<Mutex<VecDeque<WindowId>>>,
    pub(super) destroys: Arc<Mutex<VecDeque<WindowId>>>,
    pub(super) event_socket: Arc<RedoxSocket>,
    pub(super) wake_socket: Arc<TimeSocket>,
}

impl<T: 'static> EventLoopWindowTarget<T> {
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle)
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle);
        v
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Orbital(OrbitalDisplayHandle::empty())
    }
}
