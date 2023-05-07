use crate::{
    dpi::PhysicalPosition,
    event::DeviceId,
    event::{
        DeviceEvent, ElementState, Event, Force, KeyboardInput, ModifiersState, MouseButton,
        MouseScrollDelta, Touch, TouchPhase, WindowEvent,
    },
    platform_impl::{self, xkb_keymap},
    window::WindowId,
};
use input::{
    event::{
        keyboard::KeyboardEventTrait,
        pointer::PointerScrollEvent,
        tablet_pad::{ButtonState, KeyState},
        tablet_tool::{TabletToolEventTrait, TipState},
        touch::{TouchEventPosition, TouchEventSlot},
    },
    LibinputInterface,
};
use parking_lot::Mutex;
#[cfg(feature = "kms-ext")]
use std::collections::HashMap;
use std::{
    os::unix::prelude::{AsRawFd, FromRawFd, RawFd},
    path::Path,
    sync::Arc,
    time::Duration,
};

use calloop::{
    timer::TimeoutAction, EventSource, Interest, LoopHandle, Mode, Poll, PostAction, Readiness,
    RegistrationToken, Token, TokenFactory,
};
use xkbcommon::xkb;

use super::event_loop::EventSink;

pub const REPEAT_RATE: u64 = 25;
pub const REPEAT_DELAY: u64 = 600;

macro_rules! to_platform_impl {
    ($p:ident, $params:expr) => {
        $p(platform_impl::$p::Kms($params))
    };
}

macro_rules! window_id {
    () => {
        to_platform_impl!(WindowId, super::WindowId)
    };
}

macro_rules! device_id {
    () => {
        to_platform_impl!(DeviceId, super::DeviceId)
    };
}

#[cfg(feature = "kms-ext")]
pub struct Interface(pub libseat::Seat, pub HashMap<RawFd, i32>);
#[cfg(not(feature = "kms-ext"))]
pub struct Interface;

#[cfg(feature = "kms-ext")]
impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, _flags: i32) -> Result<RawFd, i32> {
        self.0
            .open_device(&path)
            .map(|(id, file)| {
                self.1.insert(file, id);
                file
            })
            .map_err(|err| err.into())
    }

    fn close_restricted(&mut self, fd: RawFd) {
        if let Some(dev) = self.1.get(&fd).copied() {
            self.0.close_device(dev).unwrap();
        }

        unsafe { std::fs::File::from_raw_fd(fd) };
    }
}

#[cfg(not(feature = "kms-ext"))]
impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<RawFd, i32> {
        use std::os::unix::prelude::*;

        std::fs::OpenOptions::new()
            .custom_flags(flags)
            .read(flags & libc::O_RDWR != 0)
            .write((flags & libc::O_WRONLY != 0) | (flags & libc::O_RDWR != 0))
            .open(path)
            .map(|file| file.into_raw_fd())
            .map_err(|err| err.raw_os_error().unwrap())
    }
    fn close_restricted(&mut self, fd: RawFd) {
        unsafe {
            std::fs::File::from_raw_fd(fd);
        }
    }
}

pub struct LibinputInputBackend {
    context: input::Libinput,
    xkb_ctx: xkb::State,
    xkb_keymap: xkb::Keymap,
    xkb_compose: xkb::compose::State,
    token: Token,
    touch_location: PhysicalPosition<f64>,
    screen_size: (u32, u32),
    modifiers: ModifiersState,
    cursor_positon: Arc<Mutex<PhysicalPosition<f64>>>,
    ev_handle: calloop::LoopHandle<'static, EventSink>,
    current_repeat: Option<RegistrationToken>,
}

impl LibinputInputBackend {
    /// Initialize a new [`LibinputInputBackend`] from a given already initialized
    /// [libinput context](input::Libinput).
    pub fn new(
        context: input::Libinput,
        screen_size: (u32, u32),
        ev_handle: calloop::LoopHandle<'static, EventSink>,
        xkb_ctx: xkb::State,
        xkb_keymap: xkb::Keymap,
        xkb_compose: xkb::compose::State,
        cursor_positon: Arc<Mutex<PhysicalPosition<f64>>>,
    ) -> Self {
        LibinputInputBackend {
            context,
            token: Token::invalid(),
            touch_location: PhysicalPosition::new(0.0, 0.0),
            modifiers: ModifiersState::empty(),
            cursor_positon,
            screen_size,
            ev_handle,
            current_repeat: None,
            xkb_ctx,
            xkb_keymap,
            xkb_compose,
        }
    }
}

impl AsRawFd for LibinputInputBackend {
    fn as_raw_fd(&self) -> RawFd {
        self.context.as_raw_fd()
    }
}

macro_rules! handle_device_event {
    ($ev:expr,$callback:expr) => {
        match $ev {
            input::event::DeviceEvent::Added(_) => {
                $callback(
                    Event::DeviceEvent {
                        device_id: device_id!(),
                        event: DeviceEvent::Added,
                    },
                    &mut (),
                );
            }
            input::event::DeviceEvent::Removed(_) => {
                $callback(
                    Event::DeviceEvent {
                        device_id: device_id!(),
                        event: DeviceEvent::Removed,
                    },
                    &mut (),
                );
            }
            _ => {}
        }
    };
}

macro_rules! handle_touch_event {
    ($self:expr,$ev:expr,$callback:expr) => {
        match $ev {
            input::event::TouchEvent::Up(e) => $callback(
                Event::WindowEvent {
                    window_id: window_id!(),
                    event: WindowEvent::Touch(Touch {
                        device_id: device_id!(),
                        phase: TouchPhase::Ended,
                        location: $self.touch_location,
                        force: None,
                        id: e.slot().unwrap() as u64,
                    }),
                },
                &mut (),
            ),
            input::event::TouchEvent::Down(e) => {
                $self.touch_location.x = e.x_transformed($self.screen_size.0);
                $self.touch_location.y = e.y_transformed($self.screen_size.1);

                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::Touch(Touch {
                            device_id: device_id!(),
                            phase: TouchPhase::Started,
                            location: $self.touch_location,
                            force: None,
                            id: e.slot().unwrap() as u64,
                        }),
                    },
                    &mut (),
                )
            }
            input::event::TouchEvent::Motion(e) => {
                $self.touch_location.x = e.x_transformed($self.screen_size.0);
                $self.touch_location.y = e.y_transformed($self.screen_size.1);

                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::Touch(Touch {
                            device_id: device_id!(),
                            phase: TouchPhase::Moved,
                            location: $self.touch_location,
                            force: None,
                            id: e.slot().unwrap() as u64,
                        }),
                    },
                    &mut (),
                );
            }
            input::event::TouchEvent::Cancel(e) => $callback(
                Event::WindowEvent {
                    window_id: window_id!(),
                    event: WindowEvent::Touch(Touch {
                        device_id: device_id!(),
                        phase: TouchPhase::Cancelled,
                        location: $self.touch_location,
                        force: None,
                        id: e.slot().unwrap() as u64,
                    }),
                },
                &mut (),
            ),
            input::event::TouchEvent::Frame(_) => $callback(
                Event::WindowEvent {
                    window_id: window_id!(),
                    event: WindowEvent::Touch(Touch {
                        device_id: device_id!(),
                        phase: TouchPhase::Ended,
                        location: $self.touch_location,
                        force: None,
                        id: 0, // e.slot().unwrap() as u64,
                    }),
                },
                &mut (),
            ),
            _ => {}
        }
    };
}

macro_rules! handle_tablet_tool_event {
    ($self:expr,$ev:expr,$callback:expr) => {
        match $ev {
            input::event::TabletToolEvent::Tip(e) => $callback(
                Event::WindowEvent {
                    window_id: window_id!(),
                    event: WindowEvent::Touch(Touch {
                        device_id: device_id!(),
                        phase: match e.tip_state() {
                            TipState::Down => TouchPhase::Started,
                            TipState::Up => TouchPhase::Ended,
                        },
                        location: PhysicalPosition::new(
                            e.x_transformed($self.screen_size.0),
                            e.y_transformed($self.screen_size.1),
                        ),
                        force: Some(Force::Calibrated {
                            force: e.pressure(),
                            max_possible_force: 1.0,
                            altitude_angle: None,
                        }),
                        id: 0,
                    }),
                },
                &mut (),
            ),
            input::event::TabletToolEvent::Button(e) => {
                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::MouseInput {
                            device_id: device_id!(),
                            state: match e.button_state() {
                                ButtonState::Pressed => ElementState::Pressed,
                                ButtonState::Released => ElementState::Released,
                            },
                            button: match e.button() {
                                1 => MouseButton::Right,
                                2 => MouseButton::Middle,
                                _ => MouseButton::Left,
                            },
                            modifiers: $self.modifiers,
                        },
                    },
                    &mut (),
                );

                $callback(
                    Event::DeviceEvent {
                        device_id: device_id!(),
                        event: DeviceEvent::Button {
                            button: e.button(),
                            state: match e.button_state() {
                                ButtonState::Pressed => ElementState::Pressed,
                                ButtonState::Released => ElementState::Released,
                            },
                        },
                    },
                    &mut (),
                );
            }
            _ => {}
        }
    };
}

macro_rules! handle_pointer_event {
    ($self:expr,$ev:expr,$callback:expr) => {
        match $ev {
            input::event::PointerEvent::Motion(e) => {
                let mut lock = $self.cursor_positon.lock();

                lock.x += e.dx();
                lock.x = lock.x.clamp(0.0, $self.screen_size.0 as f64);

                lock.y += e.dy();
                lock.y = lock.y.clamp(0.0, $self.screen_size.1 as f64);

                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::CursorMoved {
                            device_id: device_id!(),
                            position: *lock,
                            modifiers: $self.modifiers,
                        },
                    },
                    &mut (),
                );

                $callback(
                    Event::DeviceEvent {
                        device_id: device_id!(),
                        event: DeviceEvent::MouseMotion {
                            delta: (e.dx(), e.dy()),
                        },
                    },
                    &mut (),
                );
            }

            input::event::PointerEvent::Button(e) => {
                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::MouseInput {
                            device_id: device_id!(),
                            state: match e.button_state() {
                                ButtonState::Pressed => ElementState::Pressed,
                                ButtonState::Released => ElementState::Released,
                            },
                            button: match e.button() {
                                1 => MouseButton::Right,
                                2 => MouseButton::Middle,
                                _ => MouseButton::Left,
                            },
                            modifiers: $self.modifiers,
                        },
                    },
                    &mut (),
                );

                $callback(
                    Event::DeviceEvent {
                        device_id: device_id!(),
                        event: DeviceEvent::Button {
                            button: e.button(),
                            state: match e.button_state() {
                                ButtonState::Pressed => ElementState::Pressed,
                                ButtonState::Released => ElementState::Released,
                            },
                        },
                    },
                    &mut (),
                );
            }

            input::event::PointerEvent::ScrollWheel(e) => {
                use input::event::pointer::Axis;

                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::MouseWheel {
                            device_id: device_id!(),
                            delta: MouseScrollDelta::LineDelta(
                                if e.has_axis(Axis::Horizontal) {
                                    e.scroll_value(Axis::Horizontal) as f32
                                } else {
                                    0.0
                                },
                                if e.has_axis(Axis::Vertical) {
                                    e.scroll_value(Axis::Vertical) as f32
                                } else {
                                    0.0
                                },
                            ),
                            phase: TouchPhase::Moved,
                            modifiers: $self.modifiers,
                        },
                    },
                    &mut (),
                );
            }

            input::event::PointerEvent::ScrollFinger(e) => {
                use input::event::pointer::Axis;

                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::MouseWheel {
                            device_id: device_id!(),
                            delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(
                                if e.has_axis(Axis::Horizontal) {
                                    e.scroll_value(Axis::Horizontal)
                                } else {
                                    0.0
                                },
                                if e.has_axis(Axis::Vertical) {
                                    e.scroll_value(Axis::Vertical)
                                } else {
                                    0.0
                                },
                            )),
                            phase: TouchPhase::Moved,
                            modifiers: $self.modifiers,
                        },
                    },
                    &mut (),
                );
            }

            input::event::PointerEvent::MotionAbsolute(e) => {
                let mut lock = $self.cursor_positon.lock();

                lock.x = e.absolute_x_transformed($self.screen_size.0);

                lock.y = e.absolute_y_transformed($self.screen_size.1);

                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::CursorMoved {
                            device_id: device_id!(),
                            position: *lock,
                            modifiers: $self.modifiers,
                        },
                    },
                    &mut (),
                );
            }
            _ => {}
        }
    };
}

// This is used so that when you hold down a key, the same `KeyboardInput` event will be
// repeated until the key is released or another key is pressed down
fn repeat_keyboard_event(
    event: (KeyboardInput, Option<char>),
    loop_handle: &LoopHandle<'static, EventSink>,
) -> calloop::Result<RegistrationToken> {
    let timer = calloop::timer::Timer::immediate();
    let window_id = winid();
    loop_handle
        .insert_source(timer, move |_, _, data| {
            data.push(Event::WindowEvent {
                window_id,
                event: WindowEvent::KeyboardInput {
                    device_id: DeviceId(platform_impl::DeviceId::Kms(super::DeviceId)),
                    input: event.0,
                    is_synthetic: false,
                },
            });

            if let Some(c) = event.1 {
                data.push(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::ReceivedCharacter(c),
                });
            }

            TimeoutAction::ToDuration(Duration::from_millis(REPEAT_RATE))
        })
        .map_err(Into::into)
}

macro_rules! handle_keyboard_event {
    ($self:expr,$ev:expr,$callback:expr) => {{
        let state = match $ev.key_state() {
            KeyState::Pressed => ElementState::Pressed,
            KeyState::Released => ElementState::Released,
        };

        let k = if let input::event::KeyboardEvent::Key(key) = $ev {
            key.key()
        } else {
            unreachable!()
        };

        let key_offset = k + 8;
        let keysym = $self.xkb_ctx.key_get_one_sym(key_offset);
        let virtual_keycode = xkb_keymap::keysym_to_vkey(keysym);

        $self.xkb_ctx.update_key(
            key_offset,
            match state {
                ElementState::Pressed => xkb::KeyDirection::Down,
                ElementState::Released => xkb::KeyDirection::Up,
            },
        );

        #[allow(deprecated)]
        let input = KeyboardInput {
            scancode: k,
            state: state.clone(),
            virtual_keycode,
            modifiers: $self.modifiers,
        };

        if let Some(repeat) = $self.current_repeat.take() {
            $self.ev_handle.remove(repeat);
        }

        $callback(
            Event::WindowEvent {
                window_id: window_id!(),
                event: WindowEvent::KeyboardInput {
                    device_id: device_id!(),
                    input,
                    is_synthetic: false,
                },
            },
            &mut (),
        );

        if let ElementState::Pressed = state {
            $self.xkb_compose.feed(keysym);

            match $self.xkb_compose.status() {
                xkb::compose::Status::Composed => {
                    if let Some(c) = $self.xkb_compose.utf8().and_then(|f| f.chars().next()) {
                        $callback(
                            Event::WindowEvent {
                                window_id: window_id!(),
                                event: WindowEvent::ReceivedCharacter(c),
                            },
                            &mut (),
                        );
                    }
                    $self.xkb_compose.reset();
                }
                xkb::compose::Status::Cancelled | xkb::compose::Status::Nothing => {
                    let should_repeat = $self.xkb_keymap.key_repeats(key_offset);
                    let ch = $self.xkb_ctx.key_get_utf8(key_offset).chars().next();

                    if should_repeat {
                        $self.current_repeat =
                            Some(repeat_keyboard_event((input, ch), &$self.ev_handle)?);
                    }

                    if let Some(c) = ch {
                        $callback(
                            Event::WindowEvent {
                                window_id: window_id!(),
                                event: WindowEvent::ReceivedCharacter(c),
                            },
                            &mut (),
                        );
                    }
                }
                _ => {}
            }
        }
        match keysym {
            xkb_keymap::XKB_KEY_Alt_L | xkb_keymap::XKB_KEY_Alt_R => {
                match state {
                    ElementState::Pressed => $self.modifiers |= ModifiersState::ALT,
                    ElementState::Released => $self.modifiers.remove(ModifiersState::ALT),
                }

                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::ModifiersChanged($self.modifiers),
                    },
                    &mut (),
                );
            }

            xkb_keymap::XKB_KEY_Shift_L | xkb_keymap::XKB_KEY_Shift_R => {
                match state {
                    ElementState::Pressed => $self.modifiers |= ModifiersState::SHIFT,
                    ElementState::Released => $self.modifiers.remove(ModifiersState::SHIFT),
                }

                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::ModifiersChanged($self.modifiers),
                    },
                    &mut (),
                );
            }

            xkb_keymap::XKB_KEY_Control_L | xkb_keymap::XKB_KEY_Control_R => {
                match state {
                    ElementState::Pressed => $self.modifiers |= ModifiersState::CTRL,
                    ElementState::Released => $self.modifiers.remove(ModifiersState::CTRL),
                }

                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::ModifiersChanged($self.modifiers),
                    },
                    &mut (),
                );
            }

            xkb_keymap::XKB_KEY_Meta_L | xkb_keymap::XKB_KEY_Meta_R => {
                match state {
                    ElementState::Pressed => $self.modifiers |= ModifiersState::LOGO,
                    ElementState::Released => $self.modifiers.remove(ModifiersState::LOGO),
                }

                $callback(
                    Event::WindowEvent {
                        window_id: window_id!(),
                        event: WindowEvent::ModifiersChanged($self.modifiers),
                    },
                    &mut (),
                );
            }

            xkb_keymap::XKB_KEY_Sys_Req | xkb_keymap::XKB_KEY_Print => {
                if $self.modifiers.is_empty() {
                    $callback(
                        Event::WindowEvent {
                            window_id: window_id!(),
                            event: WindowEvent::CloseRequested,
                        },
                        &mut (),
                    );
                }
            }
            _ => {}
        }
    }};
}

impl EventSource for LibinputInputBackend {
    type Event = Event<'static, ()>;
    type Metadata = ();
    type Ret = ();

    fn process_events<F>(
        &mut self,
        _: Readiness,
        token: Token,
        mut callback: F,
    ) -> std::io::Result<PostAction>
    where
        F: FnMut(Self::Event, &mut ()) -> Self::Ret,
    {
        if token == self.token {
            self.context.dispatch()?;

            for event in &mut self.context {
                match event {
                    input::Event::Device(ev) => handle_device_event!(ev, callback),
                    input::Event::Touch(ev) => handle_touch_event!(self, ev, callback),
                    input::Event::Tablet(ev) => handle_tablet_tool_event!(self, ev, callback),
                    input::Event::Pointer(ev) => handle_pointer_event!(self, ev, callback),
                    input::Event::Keyboard(ev) => handle_keyboard_event!(self, ev, callback),
                    _ => {}
                }
            }
        }
        Ok(PostAction::Continue)
    }

    fn register(&mut self, poll: &mut Poll, factory: &mut TokenFactory) -> std::io::Result<()> {
        self.token = factory.token();
        poll.register(self.as_raw_fd(), Interest::READ, Mode::Level, self.token)
    }

    fn reregister(&mut self, poll: &mut Poll, factory: &mut TokenFactory) -> std::io::Result<()> {
        self.token = factory.token();
        poll.reregister(self.as_raw_fd(), Interest::READ, Mode::Level, self.token)
    }

    fn unregister(&mut self, poll: &mut Poll) -> std::io::Result<()> {
        self.token = Token::invalid();
        poll.unregister(self.as_raw_fd())
    }
}
