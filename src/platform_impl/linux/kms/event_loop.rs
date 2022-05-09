use std::{
    collections::{HashMap, VecDeque},
    marker::PhantomData,
    os::unix::prelude::{AsRawFd, FromRawFd, RawFd},
    path::Path,
    sync::{atomic::AtomicBool, mpsc::SendError, Arc},
    time::{Duration, Instant},
};
use udev::Enumerator;
use xkbcommon::xkb;

use calloop::{EventSource, Interest, Mode, Poll, PostAction, Readiness, Token, TokenFactory};
use drm::control::{property, Device, ModeTypeFlags, ResourceHandle};
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

use crate::{
    dpi::PhysicalPosition,
    event::{ElementState, Force, KeyboardInput, ModifiersState, MouseScrollDelta, StartCause},
    event_loop::{ControlFlow, EventLoopClosed},
    platform::unix::Card,
    platform_impl::{platform::sticky_exit_callback, xkb_keymap},
};

struct Interface(libseat::Seat, HashMap<RawFd, i32>);

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

pub struct LibinputInputBackend {
    context: input::Libinput,
    xkb_ctx: xkb::State,
    xkb_keymap: xkb::Keymap,
    xkb_compose: xkb::compose::State,
    token: Token,
    touch_location: PhysicalPosition<f64>,
    screen_size: (u32, u32),
    modifiers: ModifiersState,
    cursor_positon: PhysicalPosition<f64>,
    timer_handle: calloop::timer::TimerHandle<(KeyboardInput, Option<char>)>,
    // cursor_plane: drm::control::plane::Handle,
    // cursor_buffer: drm::control::framebuffer::Handle,
}

impl LibinputInputBackend {
    /// Initialize a new [`LibinputInputBackend`] from a given already initialized
    /// [libinput context](libinput::Libinput).
    pub fn new(
        context: input::Libinput,
        screen_size: (u32, u32),
        timer_handle: calloop::timer::TimerHandle<(KeyboardInput, Option<char>)>,
        xkb_ctx: xkb::State,
        xkb_keymap: xkb::Keymap,
        xkb_compose: xkb::compose::State,
        // cursor_plane: drm::control::plane::Handle,
        // cursor_buffer: drm::control::framebuffer::Handle
    ) -> Self {
        LibinputInputBackend {
            context,
            token: Token::invalid(),
            touch_location: PhysicalPosition::new(0.0, 0.0),
            cursor_positon: PhysicalPosition::new(0.0, 0.0),
            modifiers: ModifiersState::empty(),
            screen_size,
            timer_handle,
            xkb_ctx,
            xkb_keymap,
            xkb_compose,
            // cursor_buffer,
            // cursor_plane,
        }
    }
}

impl AsRawFd for LibinputInputBackend {
    fn as_raw_fd(&self) -> RawFd {
        self.context.as_raw_fd()
    }
}

impl EventSource for LibinputInputBackend {
    type Event = crate::event::Event<'static, ()>;
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
                    input::Event::Device(ev) => match ev {
                        input::event::DeviceEvent::Added(_) => {
                            callback(
                                crate::event::Event::DeviceEvent {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                    ),
                                    event: crate::event::DeviceEvent::Added,
                                },
                                &mut (),
                            );
                        }
                        input::event::DeviceEvent::Removed(_) => {
                            callback(
                                crate::event::Event::DeviceEvent {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                    ),
                                    event: crate::event::DeviceEvent::Removed,
                                },
                                &mut (),
                            );
                        }
                        _ => {}
                    },
                    input::Event::Touch(ev) => match ev {
                        input::event::TouchEvent::Up(e) => callback(
                            crate::event::Event::WindowEvent {
                                window_id: crate::window::WindowId(
                                    crate::platform_impl::WindowId::Kms(super::WindowId),
                                ),
                                event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                    ),
                                    phase: crate::event::TouchPhase::Ended,
                                    location: self.touch_location,
                                    force: None,
                                    id: e.slot().unwrap() as u64,
                                }),
                            },
                            &mut (),
                        ),
                        input::event::TouchEvent::Down(e) => {
                            self.touch_location.x = e.x_transformed(self.screen_size.0);
                            self.touch_location.y = e.y_transformed(self.screen_size.1);
                            callback(
                                crate::event::Event::WindowEvent {
                                    window_id: crate::window::WindowId(
                                        crate::platform_impl::WindowId::Kms(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                        ),
                                        phase: crate::event::TouchPhase::Started,
                                        location: self.touch_location,
                                        force: None,
                                        id: e.slot().unwrap() as u64,
                                    }),
                                },
                                &mut (),
                            )
                        }
                        input::event::TouchEvent::Motion(e) => {
                            self.touch_location.x = e.x_transformed(self.screen_size.0);
                            self.touch_location.y = e.y_transformed(self.screen_size.1);
                            callback(
                                crate::event::Event::WindowEvent {
                                    window_id: crate::window::WindowId(
                                        crate::platform_impl::WindowId::Kms(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                        ),
                                        phase: crate::event::TouchPhase::Moved,
                                        location: self.touch_location,
                                        force: None,
                                        id: e.slot().unwrap() as u64,
                                    }),
                                },
                                &mut (),
                            );
                        }
                        input::event::TouchEvent::Cancel(e) => callback(
                            crate::event::Event::WindowEvent {
                                window_id: crate::window::WindowId(
                                    crate::platform_impl::WindowId::Kms(super::WindowId),
                                ),
                                event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                    ),
                                    phase: crate::event::TouchPhase::Cancelled,
                                    location: self.touch_location,
                                    force: None,
                                    id: e.slot().unwrap() as u64,
                                }),
                            },
                            &mut (),
                        ),
                        input::event::TouchEvent::Frame(_) => callback(
                            crate::event::Event::WindowEvent {
                                window_id: crate::window::WindowId(
                                    crate::platform_impl::WindowId::Kms(super::WindowId),
                                ),
                                event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                    ),
                                    phase: crate::event::TouchPhase::Ended,
                                    location: self.touch_location,
                                    force: None,
                                    id: 0, // e.slot().unwrap() as u64,
                                }),
                            },
                            &mut (),
                        ),
                        _ => {}
                    },
                    input::Event::Tablet(ev) => match ev {
                        input::event::TabletToolEvent::Tip(e) => callback(
                            crate::event::Event::WindowEvent {
                                window_id: crate::window::WindowId(
                                    crate::platform_impl::WindowId::Kms(super::WindowId),
                                ),
                                event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                    ),
                                    phase: match e.tip_state() {
                                        TipState::Down => crate::event::TouchPhase::Started,
                                        TipState::Up => crate::event::TouchPhase::Ended,
                                    },
                                    location: PhysicalPosition::new(
                                        e.x_transformed(self.screen_size.0),
                                        e.y_transformed(self.screen_size.1),
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
                            callback(
                                crate::event::Event::WindowEvent {
                                    window_id: crate::window::WindowId(
                                        crate::platform_impl::WindowId::Kms(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::MouseInput {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                        ),
                                        state: match e.button_state() {
                                            ButtonState::Pressed => {
                                                crate::event::ElementState::Pressed
                                            }
                                            ButtonState::Released => {
                                                crate::event::ElementState::Released
                                            }
                                        },
                                        button: match e.button() {
                                            1 => crate::event::MouseButton::Right,
                                            2 => crate::event::MouseButton::Middle,
                                            _ => crate::event::MouseButton::Left,
                                        },
                                        modifiers: self.modifiers,
                                    },
                                },
                                &mut (),
                            );
                            callback(
                                crate::event::Event::DeviceEvent {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                    ),
                                    event: crate::event::DeviceEvent::Button {
                                        button: e.button(),
                                        state: match e.button_state() {
                                            ButtonState::Pressed => {
                                                crate::event::ElementState::Pressed
                                            }
                                            ButtonState::Released => {
                                                crate::event::ElementState::Released
                                            }
                                        },
                                    },
                                },
                                &mut (),
                            );
                        }
                        _ => {}
                    },
                    input::Event::Pointer(e) => match e {
                        input::event::PointerEvent::Motion(e) => {
                            self.cursor_positon.x += e.dx();
                            self.cursor_positon.x =
                                self.cursor_positon.x.clamp(0.0, self.screen_size.0 as f64);
                            self.cursor_positon.y += e.dy();
                            self.cursor_positon.y =
                                self.cursor_positon.y.clamp(0.0, self.screen_size.1 as f64);
                            callback(
                                crate::event::Event::WindowEvent {
                                    window_id: crate::window::WindowId(
                                        crate::platform_impl::WindowId::Kms(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::CursorMoved {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                        ),
                                        position: self.cursor_positon,
                                        modifiers: self.modifiers,
                                    },
                                },
                                &mut (),
                            );
                            callback(
                                crate::event::Event::DeviceEvent {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                    ),
                                    event: crate::event::DeviceEvent::MouseMotion {
                                        delta: (e.dx(), e.dy()),
                                    },
                                },
                                &mut (),
                            );
                        }
                        input::event::PointerEvent::Button(e) => {
                            callback(
                                crate::event::Event::WindowEvent {
                                    window_id: crate::window::WindowId(
                                        crate::platform_impl::WindowId::Kms(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::MouseInput {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                        ),
                                        state: match e.button_state() {
                                            ButtonState::Pressed => {
                                                crate::event::ElementState::Pressed
                                            }
                                            ButtonState::Released => {
                                                crate::event::ElementState::Released
                                            }
                                        },
                                        button: match e.button() {
                                            1 => crate::event::MouseButton::Right,
                                            2 => crate::event::MouseButton::Middle,
                                            _ => crate::event::MouseButton::Left,
                                        },
                                        modifiers: self.modifiers,
                                    },
                                },
                                &mut (),
                            );
                            callback(
                                crate::event::Event::DeviceEvent {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                    ),
                                    event: crate::event::DeviceEvent::Button {
                                        button: e.button(),
                                        state: match e.button_state() {
                                            ButtonState::Pressed => {
                                                crate::event::ElementState::Pressed
                                            }
                                            ButtonState::Released => {
                                                crate::event::ElementState::Released
                                            }
                                        },
                                    },
                                },
                                &mut (),
                            );
                        }
                        input::event::PointerEvent::ScrollWheel(e) => {
                            callback(
                                crate::event::Event::WindowEvent {
                                    window_id: crate::window::WindowId(
                                        crate::platform_impl::WindowId::Kms(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::MouseWheel {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                        ),
                                        delta: MouseScrollDelta::LineDelta(
                                            if e.has_axis(input::event::pointer::Axis::Horizontal) {
                                                e.scroll_value(
                                                    input::event::pointer::Axis::Horizontal,
                                                )
                                                    as f32
                                            } else {
                                                0.0
                                            },
                                            if e.has_axis(input::event::pointer::Axis::Vertical) {
                                                e.scroll_value(
                                                    input::event::pointer::Axis::Vertical,
                                                )
                                                    as f32
                                            } else {
                                                0.0
                                            },
                                        ),
                                        phase: crate::event::TouchPhase::Moved,
                                        modifiers: self.modifiers,
                                    },
                                },
                                &mut (),
                            );
                        }
                        input::event::PointerEvent::ScrollFinger(e) => {
                            callback(
                                crate::event::Event::WindowEvent {
                                    window_id: crate::window::WindowId(
                                        crate::platform_impl::WindowId::Kms(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::MouseWheel {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                        ),
                                        delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(
                                            if e.has_axis(input::event::pointer::Axis::Horizontal) {
                                                e.scroll_value(
                                                    input::event::pointer::Axis::Horizontal,
                                                )
                                            } else {
                                                0.0
                                            },
                                            if e.has_axis(input::event::pointer::Axis::Vertical) {
                                                e.scroll_value(
                                                    input::event::pointer::Axis::Vertical,
                                                )
                                            } else {
                                                0.0
                                            },
                                        )),
                                        phase: crate::event::TouchPhase::Moved,
                                        modifiers: self.modifiers,
                                    },
                                },
                                &mut (),
                            );
                        }
                        input::event::PointerEvent::MotionAbsolute(e) => {
                            self.cursor_positon.x = e.absolute_x_transformed(self.screen_size.0);
                            self.cursor_positon.y = e.absolute_y_transformed(self.screen_size.1);
                            callback(
                                crate::event::Event::WindowEvent {
                                    window_id: crate::window::WindowId(
                                        crate::platform_impl::WindowId::Kms(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::CursorMoved {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                        ),
                                        position: self.cursor_positon,
                                        modifiers: self.modifiers,
                                    },
                                },
                                &mut (),
                            );
                        }
                        _ => {}
                    },
                    input::Event::Keyboard(ev) => {
                        let state = match ev.key_state() {
                            KeyState::Pressed => crate::event::ElementState::Pressed,
                            KeyState::Released => crate::event::ElementState::Released,
                        };
                        let k = if let input::event::KeyboardEvent::Key(key) = ev {
                            key.key()
                        } else {
                            unreachable!()
                        };
                        let key_offset = k + 8;
                        let keysym = self.xkb_ctx.key_get_one_sym(key_offset);
                        let virtual_keycode = xkb_keymap::keysym_to_vkey(keysym);
                        self.xkb_ctx.update_key(
                            key_offset,
                            match state {
                                ElementState::Pressed => xkb::KeyDirection::Down,
                                ElementState::Released => xkb::KeyDirection::Up,
                            },
                        );
                        let input = KeyboardInput {
                            scancode: k,
                            state: state.clone(),
                            virtual_keycode,
                            modifiers: self.modifiers,
                        };
                        self.timer_handle.cancel_all_timeouts();
                        callback(
                            crate::event::Event::WindowEvent {
                                window_id: crate::window::WindowId(
                                    crate::platform_impl::WindowId::Kms(super::WindowId),
                                ),
                                event: crate::event::WindowEvent::KeyboardInput {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Kms(super::DeviceId),
                                    ),
                                    input,
                                    is_synthetic: false,
                                },
                            },
                            &mut (),
                        );
                        if let crate::event::ElementState::Pressed = state {
                            self.xkb_compose.feed(keysym);
                            match self.xkb_compose.status() {
                                xkb::compose::Status::Composed => {
                                    if let Some(c) =
                                        self.xkb_compose.utf8().and_then(|f| f.chars().next())
                                    {
                                        callback(
                                            crate::event::Event::WindowEvent {
                                                window_id: crate::window::WindowId(
                                                    crate::platform_impl::WindowId::Kms(
                                                        super::WindowId,
                                                    ),
                                                ),
                                                event: crate::event::WindowEvent::ReceivedCharacter(
                                                    c,
                                                ),
                                            },
                                            &mut (),
                                        );
                                    }
                                }
                                xkb::compose::Status::Cancelled => {
                                    let should_repeat = self.xkb_keymap.key_repeats(key_offset);
                                    let ch = self.xkb_ctx.key_get_utf8(key_offset).chars().next();
                                    if should_repeat {
                                        self.timer_handle
                                            .add_timeout(Duration::from_millis(600), (input, ch));
                                    }
                                    if let Some(c) = ch {
                                        callback(
                                            crate::event::Event::WindowEvent {
                                                window_id: crate::window::WindowId(
                                                    crate::platform_impl::WindowId::Kms(
                                                        super::WindowId,
                                                    ),
                                                ),
                                                event: crate::event::WindowEvent::ReceivedCharacter(
                                                    c,
                                                ),
                                            },
                                            &mut (),
                                        );
                                    }
                                }
                                xkb::compose::Status::Nothing => {
                                    let should_repeat = self.xkb_keymap.key_repeats(key_offset);
                                    let ch = self.xkb_ctx.key_get_utf8(key_offset).chars().next();
                                    if should_repeat {
                                        self.timer_handle
                                            .add_timeout(Duration::from_millis(600), (input, ch));
                                    }
                                    if let Some(c) = ch {
                                        callback(
                                            crate::event::Event::WindowEvent {
                                                window_id: crate::window::WindowId(
                                                    crate::platform_impl::WindowId::Kms(
                                                        super::WindowId,
                                                    ),
                                                ),
                                                event: crate::event::WindowEvent::ReceivedCharacter(
                                                    c,
                                                ),
                                            },
                                            &mut (),
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                        match keysym {
                                xkb_keymap::XKB_KEY_Alt_L
                                    | xkb_keymap::XKB_KEY_Alt_R
                                    => {
                                        match state {
                                            ElementState::Pressed => self.modifiers |= ModifiersState::ALT,
                                            ElementState::Released => self.modifiers.remove(ModifiersState::ALT)
                                        }
                                        callback(crate::event::Event::WindowEvent {
                                            window_id: crate::window::WindowId(crate::platform_impl::WindowId::Kms(super::WindowId)),
                                            event:crate::event::WindowEvent::ModifiersChanged(self.modifiers)}, &mut ());
                                    }
                                | xkb_keymap::XKB_KEY_Shift_L // LShift
                                    | xkb_keymap::XKB_KEY_Shift_R // RShift
                                    => {
                                        match state {
                                            ElementState::Pressed => self.modifiers |= ModifiersState::SHIFT,
                                            ElementState::Released => self.modifiers.remove(ModifiersState::SHIFT)
                                        }
                                        callback(crate::event::Event::WindowEvent {
                                            window_id: crate::window::WindowId(crate::platform_impl::WindowId::Kms(super::WindowId)),
                                            event:crate::event::WindowEvent::ModifiersChanged(self.modifiers)}, &mut ());
                                    }

                                | xkb_keymap::XKB_KEY_Control_L // LCtrl
                                    | xkb_keymap::XKB_KEY_Control_R // RCtrl
                                    => {
                                        match state {
                                            ElementState::Pressed => self.modifiers |= ModifiersState::CTRL,
                                            ElementState::Released => self.modifiers.remove(ModifiersState::CTRL)
                                        }
                                        callback(crate::event::Event::WindowEvent {
                                            window_id: crate::window::WindowId(crate::platform_impl::WindowId::Kms(super::WindowId)),
                                            event:crate::event::WindowEvent::ModifiersChanged(self.modifiers)}, &mut ());
                                    }

                                | xkb_keymap::XKB_KEY_Meta_L // LMeta
                                    | xkb_keymap::XKB_KEY_Meta_R // RMeta
                                    => {
                                        match state {
                                            ElementState::Pressed => self.modifiers |= ModifiersState::LOGO,
                                            ElementState::Released => self.modifiers.remove(ModifiersState::LOGO)
                                        }
                                        callback(crate::event::Event::WindowEvent {
                                            window_id: crate::window::WindowId(crate::platform_impl::WindowId::Kms(super::WindowId)),
                                            event: crate::event::WindowEvent::ModifiersChanged(self.modifiers)}, &mut ());
                                    }
                                xkb_keymap::XKB_KEY_Sys_Req | xkb_keymap::XKB_KEY_Print
                                    => {
                                        if self.modifiers.is_empty() {
                                            callback(crate::event::Event::WindowEvent {
                                                window_id: crate::window::WindowId(crate::platform_impl::WindowId::Kms(super::WindowId)),
                                                event: crate::event::WindowEvent::CloseRequested
                                            }, &mut ());
                                        }
                                    }
                                _ => {}
                            }
                    }
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

/// An event loop's sink to deliver events from the Wayland event callbacks
/// to the winit's user.
type EventSink = Vec<crate::event::Event<'static, ()>>;

pub struct EventLoopWindowTarget<T> {
    /// drm Connector
    pub connector: drm::control::connector::Info,

    /// drm crtc
    pub crtc: drm::control::crtc::Info,

    /// drm mode
    pub mode: drm::control::Mode,

    /// drm plane
    pub plane: drm::control::plane::Handle,

    // /// drm dumbbuffer containing the cursor
    // pub cursor_buffer: drm::control::framebuffer::Handle,
    /// drm device
    pub device: Card,

    /// Event loop handle.
    pub event_loop_handle: calloop::LoopHandle<'static, EventSink>,

    pub(crate) event_sink: EventSink,

    /// A proxy to wake up event loop.
    pub event_loop_awakener: calloop::ping::Ping,

    _marker: std::marker::PhantomData<T>,
}

impl<T> EventLoopWindowTarget<T> {
    #[inline]
    pub fn primary_monitor(&self) -> Option<crate::monitor::MonitorHandle> {
        Some(crate::monitor::MonitorHandle {
            inner: crate::platform_impl::MonitorHandle::Kms(super::MonitorHandle(
                self.connector.clone(),
            )),
        })
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<super::MonitorHandle> {
        self.device
            .resource_handles()
            .unwrap()
            .connectors()
            .iter()
            .map(|f| super::MonitorHandle(self.device.get_connector(*f).unwrap()))
            .collect()
    }
}

pub struct EventLoop<T: 'static> {
    /// Event loop.
    event_loop: calloop::EventLoop<'static, EventSink>,

    /// Pending user events.
    pending_user_events: std::rc::Rc<std::cell::RefCell<Vec<T>>>,

    /// Sender of user events.
    user_events_sender: calloop::channel::Sender<T>,

    /// Window target.
    window_target: crate::event_loop::EventLoopWindowTarget<T>,
}

pub(crate) fn find_prop_id<T: ResourceHandle>(
    card: &Card,
    handle: T,
    name: &'static str,
) -> Option<property::Handle> {
    let props = card
        .get_properties(handle)
        .expect("Could not get props of connector");
    let (ids, _vals) = props.as_props_and_values();
    ids.iter()
        .find(|&id| {
            let info = card.get_property(*id).unwrap();
            info.name().to_str().map(|x| x == name).unwrap_or(false)
        })
        .cloned()
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> Result<EventLoop<T>, crate::error::OsError> {
        let mut seat = {
            let active = Arc::new(AtomicBool::new(false));
            let t_active = active.clone();
            let mut s = libseat::Seat::open(
                move |_, event| {
                    if let libseat::SeatEvent::Enable = event {
                        t_active.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                },
                None,
            )
            .map_err(|_| {
                crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::KmsMisc("Failed to open libseat"),
                )
            })?;

            while !active.load(std::sync::atomic::Ordering::SeqCst) {
                if let Err(_) = s.dispatch(-1) {
                    return Err(crate::error::OsError::new(
                        line!(),
                        file!(),
                        crate::platform_impl::OsError::KmsMisc("Failed to dispatch seat"),
                    ));
                }
            }
            s
        };
        // Safety
        //
        // This string value has the same lifetime as the seat in question, and will not be dropped
        // until the seat is, which is not before `udev_assign_seat` is run.
        let seat_name = unsafe { std::mem::transmute::<&str, &'static str>(seat.name()) };
        let card_path = std::env::var("WINIT_DRM_CARD").ok().map_or_else(
            || {
                let mut enumerator = Enumerator::new().map_err(|_| {
                    crate::error::OsError::new(
                        line!(),
                        file!(),
                        crate::platform_impl::OsError::KmsMisc("Failed to open udev enumerator"),
                    )
                })?;
                enumerator.match_subsystem("drm").map_err(|_| {
                    crate::error::OsError::new(
                        line!(),
                        file!(),
                        crate::platform_impl::OsError::KmsMisc("Failed to enumerate drm subsystem"),
                    )
                })?;
                enumerator.match_sysname("card[0-9]*").map_err(|_| {
                    crate::error::OsError::new(
                        line!(),
                        file!(),
                        crate::platform_impl::OsError::KmsMisc("Failed to find a valid card"),
                    )
                })?;
                enumerator
                    .scan_devices()
                    .map_err(|_| {
                        crate::error::OsError::new(
                            line!(),
                            file!(),
                            crate::platform_impl::OsError::KmsMisc("Failed to scan devices"),
                        )
                    })?
                    .filter(|device| {
                        let dev_seat_name = device
                            .property_value("ID_SEAT")
                            .map(|x| x.to_os_string())
                            .unwrap_or_else(|| std::ffi::OsString::from("seat0"));
                        if dev_seat_name == seat_name {
                            if let Ok(Some(pci)) = device.parent_with_subsystem(Path::new("pci")) {
                                if let Some(id) = pci.attribute_value("boot_vga") {
                                    return id == "1";
                                }
                            }
                        }
                        false
                    })
                    .flat_map(|device| device.devnode().map(std::path::PathBuf::from))
                    .next()
                    .or_else(|| {
                        enumerator
                            .scan_devices()
                            .ok()?
                            .filter(|device| {
                                device
                                    .property_value("ID_SEAT")
                                    .map(|x| x.to_os_string())
                                    .unwrap_or_else(|| std::ffi::OsString::from("seat0"))
                                    == seat_name
                            })
                            .flat_map(|device| device.devnode().map(std::path::PathBuf::from))
                            .next()
                    })
                    .ok_or(crate::error::OsError::new(
                        line!(),
                        file!(),
                        crate::platform_impl::OsError::KmsMisc("Failed to find suitable GPU"),
                    ))
            },
            |p| Ok(Into::into(p)),
        )?;
        let dev = seat.open_device(&card_path).map_err(|_| {
            crate::error::OsError::new(
                line!(),
                file!(),
                crate::platform_impl::OsError::KmsMisc("Failed to initialize DRM"),
            )
        })?;
        let drm = Card(std::sync::Arc::new(dev.1));
        let mut input = input::Libinput::new_with_udev(Interface(seat, HashMap::new()));
        input.udev_assign_seat(seat_name).unwrap();
        let xkb_ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &xkb_ctx,
            "",
            "",
            "",
            "",
            std::env::var("WINIT_XKB_OPTIONS").ok(),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or(crate::error::OsError::new(
            line!(),
            file!(),
            crate::platform_impl::OsError::KmsMisc("Failed to compile XKB keymap"),
        ))?;
        let state = xkb::State::new(&keymap);
        let compose_table = xkb::compose::Table::new_from_locale(
            &xkb_ctx,
            std::env::var_os("LC_ALL")
                .unwrap_or_else(|| {
                    std::env::var_os("LC_CTYPE").unwrap_or_else(|| {
                        std::env::var_os("LANG").unwrap_or_else(|| std::ffi::OsString::from("C"))
                    })
                })
                .as_os_str(),
            xkb::compose::COMPILE_NO_FLAGS,
        )
        .or(Err(crate::error::OsError::new(
            line!(),
            file!(),
            crate::platform_impl::OsError::KmsMisc("Failed to compile XKB compose table"),
        )))?;
        let xkb_compose = xkb::compose::State::new(&compose_table, xkb::compose::STATE_NO_FLAGS);

        drm::Device::set_client_capability(&drm, drm::ClientCapability::UniversalPlanes, true).or(
            Err(crate::error::OsError::new(
                line!(),
                file!(),
                crate::platform_impl::OsError::KmsMisc(
                    "kms device does not support universal planes",
                ),
            )),
        )?;
        drm::Device::set_client_capability(&drm, drm::ClientCapability::Atomic, true).or(Err(
            crate::error::OsError::new(
                line!(),
                file!(),
                crate::platform_impl::OsError::KmsMisc(
                    "kms device does not support atomic modesetting",
                ),
            ),
        ))?;

        // Load the information.
        let res = drm.resource_handles().or(Err(crate::error::OsError::new(
            line!(),
            file!(),
            crate::platform_impl::OsError::KmsMisc("Could not load normal resource ids."),
        )))?;
        let coninfo: Vec<drm::control::connector::Info> = res
            .connectors()
            .iter()
            .flat_map(|con| drm.get_connector(*con))
            .collect();
        let crtcinfo: Vec<drm::control::crtc::Info> = res
            .crtcs()
            .iter()
            .flat_map(|crtc| drm.get_crtc(*crtc))
            .collect();
        // Filter each connector until we find one that's connected.
        let con = coninfo
            .iter()
            .find(|&i| i.state() == drm::control::connector::State::Connected)
            .ok_or(crate::error::OsError::new(
                line!(),
                file!(),
                crate::platform_impl::OsError::KmsMisc("No connected connectors"),
            ))?;

        let crtc = crtcinfo.get(0).ok_or(crate::error::OsError::new(
            line!(),
            file!(),
            crate::platform_impl::OsError::KmsMisc("No crtcs found"),
        ))?;

        // Get the first (usually best) mode
        let &mode = con
            .modes()
            .iter()
            .find(|f| f.mode_type().contains(ModeTypeFlags::PREFERRED))
            .or(con.modes().get(0))
            .ok_or(crate::error::OsError::new(
                line!(),
                file!(),
                crate::platform_impl::OsError::KmsMisc("No modes found on connector"),
            ))?;

        let planes = drm.plane_handles().or(Err(crate::error::OsError::new(
            line!(),
            file!(),
            crate::platform_impl::OsError::KmsMisc("Could not list planes"),
        )))?;

        /*
          let mut db = drm
          .create_dumb_buffer((64, 64), drm::buffer::KmsFourcc::Xrgb8888, 32)
          .or(Err(crate::error::OsError::new(
          line!(),
          file!(),
          crate::platform_impl::OsError::KmsMisc("Could not create dumb buffer"),
          )))?;

          {
          let mut map = drm
          .map_dumb_buffer(&mut db)
          .expect("Could not map dumbbuffer");
          for b in map.as_mut() {
        *b = 128;
        }
        }

        let fb = drm
        .add_framebuffer(&db, 24, 32)
        .or(Err(crate::error::OsError::new(
        line!(),
        file!(),
        crate::platform_impl::OsError::KmsMisc("Could not create FB"),
        )))?;

        let (better_planes, compatible_planes): (
        Vec<drm::control::plane::Handle>,
        Vec<drm::control::plane::Handle>,
        ) = planes
        .planes()
        .iter()
        .filter(|&&plane| {
        drm.get_plane(plane)
        .map(|plane_info| {
        let compatible_crtcs = res.filter_crtcs(plane_info.possible_crtcs());
        compatible_crtcs.contains(&crtc.handle())
        })
        .unwrap_or(false)
        })
        .partition(|&&plane| {
        if let Ok(props) = drm.get_properties(plane) {
        let (ids, vals) = props.as_props_and_values();
        for (&id, &val) in ids.iter().zip(vals.iter()) {
        if let Ok(info) = drm.get_property(id) {
        if info.name().to_str().map(|x| x == "type").unwrap_or(false) {
        return val == (drm::control::PlaneType::Cursor as u32).into();
        }
        }
        }
        }
        false
        });
        let plane = *better_planes.get(0).unwrap_or(&compatible_planes[0]);
        */
        let (p_better_planes, p_compatible_planes): (
            Vec<drm::control::plane::Handle>,
            Vec<drm::control::plane::Handle>,
        ) = planes
            .planes()
            .iter()
            .filter(|&&plane| {
                drm.get_plane(plane)
                    .map(|plane_info| {
                        let compatible_crtcs = res.filter_crtcs(plane_info.possible_crtcs());
                        compatible_crtcs.contains(&crtc.handle())
                    })
                    .unwrap_or(false)
            })
            .partition(|&&plane| {
                if let Ok(props) = drm.get_properties(plane) {
                    let (ids, vals) = props.as_props_and_values();
                    for (&id, &val) in ids.iter().zip(vals.iter()) {
                        if let Ok(info) = drm.get_property(id) {
                            if info.name().to_str().map(|x| x == "type").unwrap_or(false) {
                                return val == (drm::control::PlaneType::Primary as u32).into();
                            }
                        }
                    }
                }
                false
            });

        let p_plane = *p_better_planes.get(0).unwrap_or(&p_compatible_planes[0]);

        let (disp_width, disp_height) = mode.size();

        let event_loop: calloop::EventLoop<'static, EventSink> =
            calloop::EventLoop::try_new().unwrap();

        let handle = event_loop.handle();

        // A source of user events.
        let pending_user_events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let pending_user_events_clone = pending_user_events.clone();
        let (user_events_sender, user_events_channel) = calloop::channel::channel();

        // User events channel.
        handle
            .insert_source(user_events_channel, move |event, _, _| {
                if let calloop::channel::Event::Msg(msg) = event {
                    pending_user_events_clone.borrow_mut().push(msg);
                }
            })
            .unwrap();

        // An event's loop awakener to wake up for window events from winit's windows.
        let (event_loop_awakener, event_loop_awakener_source) = calloop::ping::make_ping().unwrap();

        let event_sink = EventSink::new();

        // Handler of window requests.
        handle
            .insert_source(
                event_loop_awakener_source,
                move |_event, _metadata, data| {
                    data.push(crate::event::Event::RedrawRequested(
                        crate::window::WindowId(crate::platform_impl::WindowId::Kms(
                            super::WindowId,
                        )),
                    ));
                },
            )
            .unwrap();

        let repeat_handler = calloop::timer::Timer::new().unwrap();

        let repeat_handle = repeat_handler.handle();

        let repeat_loop: calloop::Dispatcher<
            'static,
            calloop::timer::Timer<(KeyboardInput, Option<char>)>,
            EventSink,
        > = calloop::Dispatcher::new(
            repeat_handler,
            move |event, metadata, data: &mut EventSink| {
                data.push(crate::event::Event::WindowEvent {
                    window_id: crate::window::WindowId(crate::platform_impl::WindowId::Kms(
                        super::WindowId,
                    )),
                    event: crate::event::WindowEvent::KeyboardInput {
                        device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Kms(
                            super::DeviceId,
                        )),
                        input: event.0,
                        is_synthetic: false,
                    },
                });
                if let Some(c) = event.1 {
                    data.push(crate::event::Event::WindowEvent {
                        window_id: crate::window::WindowId(crate::platform_impl::WindowId::Kms(
                            super::WindowId,
                        )),
                        event: crate::event::WindowEvent::ReceivedCharacter(c),
                    });
                }
                metadata.add_timeout(Duration::from_millis(25), event);
            },
        );

        let input_backend: LibinputInputBackend = LibinputInputBackend::new(
            input,
            (disp_width.into(), disp_height.into()), // plane, fb
            repeat_handle,
            state,
            keymap,
            xkb_compose,
        );

        let input_loop: calloop::Dispatcher<'static, LibinputInputBackend, EventSink> =
            calloop::Dispatcher::new(
                input_backend,
                move |event, _metadata, data: &mut EventSink| {
                    data.push(event);
                },
            );

        handle.register_dispatcher(input_loop).unwrap();
        handle.register_dispatcher(repeat_loop).unwrap();

        let window_target = crate::event_loop::EventLoopWindowTarget {
            p: crate::platform_impl::EventLoopWindowTarget::Kms(EventLoopWindowTarget {
                connector: con.clone(),
                crtc: crtc.clone(),
                device: drm,
                plane: p_plane,
                // cursor_buffer: fb,
                mode,
                event_loop_handle: handle,
                event_sink,
                event_loop_awakener,
                _marker: PhantomData,
            }),
            _marker: PhantomData,
        };

        Ok(EventLoop {
            event_loop,
            pending_user_events,
            user_events_sender,
            window_target,
        })
    }

    pub fn run<F>(mut self, callback: F) -> !
    where
        F: FnMut(
                crate::event::Event<'_, T>,
                &crate::event_loop::EventLoopWindowTarget<T>,
                &mut ControlFlow,
            ) + 'static,
    {
        let exit_code = self.run_return(callback);
        std::process::exit(exit_code);
    }

    pub fn run_return<F>(&mut self, mut callback: F) -> i32
    where
        F: FnMut(
            crate::event::Event<'_, T>,
            &crate::event_loop::EventLoopWindowTarget<T>,
            &mut ControlFlow,
        ),
    {
        let mut control_flow = ControlFlow::Poll;
        let pending_user_events = self.pending_user_events.clone();
        let mut event_sink_back_buffer = Vec::new();

        callback(
            crate::event::Event::NewEvents(StartCause::Init),
            &self.window_target,
            &mut control_flow,
        );

        callback(
            crate::event::Event::RedrawRequested(crate::window::WindowId(
                crate::platform_impl::WindowId::Kms(super::WindowId),
            )),
            &self.window_target,
            &mut control_flow,
        );

        let exit_code = loop {
            match control_flow {
                ControlFlow::ExitWithCode(code) => break code,
                ControlFlow::Poll => {
                    // Non-blocking dispatch.
                    let timeout = Duration::from_millis(0);
                    if let Err(error) = self.loop_dispatch(Some(timeout)) {
                        break error.raw_os_error().unwrap_or(1);
                    }

                    callback(
                        crate::event::Event::NewEvents(StartCause::Poll),
                        &self.window_target,
                        &mut control_flow,
                    );
                }
                ControlFlow::Wait => {
                    if let Err(error) = self.loop_dispatch(None) {
                        break error.raw_os_error().unwrap_or(1);
                    }

                    callback(
                        crate::event::Event::NewEvents(StartCause::WaitCancelled {
                            start: Instant::now(),
                            requested_resume: None,
                        }),
                        &self.window_target,
                        &mut control_flow,
                    );
                }
                ControlFlow::WaitUntil(deadline) => {
                    let start = Instant::now();

                    // Compute the amount of time we'll block for.
                    let duration = if deadline > start {
                        deadline - start
                    } else {
                        Duration::from_millis(0)
                    };

                    if let Err(error) = self.loop_dispatch(Some(duration)) {
                        break error.raw_os_error().unwrap_or(1);
                    }

                    let now = Instant::now();

                    if now < deadline {
                        callback(
                            crate::event::Event::NewEvents(StartCause::WaitCancelled {
                                start,
                                requested_resume: Some(deadline),
                            }),
                            &self.window_target,
                            &mut control_flow,
                        )
                    } else {
                        callback(
                            crate::event::Event::NewEvents(StartCause::ResumeTimeReached {
                                start,
                                requested_resume: deadline,
                            }),
                            &self.window_target,
                            &mut control_flow,
                        )
                    }
                }
            }

            // Handle pending user events. We don't need back buffer, since we can't dispatch
            // user events indirectly via callback to the user.
            for user_event in pending_user_events.borrow_mut().drain(..) {
                sticky_exit_callback(
                    crate::event::Event::UserEvent(user_event),
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            // The purpose of the back buffer and that swap is to not hold borrow_mut when
            // we're doing callback to the user, since we can double borrow if the user decides
            // to create a window in one of those callbacks.
            self.with_window_target(|window_target| {
                let state = &mut window_target.event_sink;
                std::mem::swap::<Vec<crate::event::Event<'static, ()>>>(
                    &mut event_sink_back_buffer,
                    state,
                );
            });

            // Handle pending window events.
            for event in event_sink_back_buffer.drain(..) {
                let event = event.map_nonuser_event().unwrap();
                sticky_exit_callback(event, &self.window_target, &mut control_flow, &mut callback);
            }

            // Send events cleared.
            sticky_exit_callback(
                crate::event::Event::MainEventsCleared,
                &self.window_target,
                &mut control_flow,
                &mut callback,
            );

            // Send RedrawEventCleared.
            sticky_exit_callback(
                crate::event::Event::RedrawEventsCleared,
                &self.window_target,
                &mut control_flow,
                &mut callback,
            );
        };

        callback(
            crate::event::Event::LoopDestroyed,
            &self.window_target,
            &mut control_flow,
        );
        exit_code
    }

    #[inline]
    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::new(self.user_events_sender.clone())
    }

    #[inline]
    pub fn window_target(&self) -> &crate::event_loop::EventLoopWindowTarget<T> {
        &self.window_target
    }

    fn with_window_target<U, F: FnOnce(&mut EventLoopWindowTarget<T>) -> U>(&mut self, f: F) -> U {
        let state = match &mut self.window_target.p {
            crate::platform_impl::EventLoopWindowTarget::Kms(window_target) => window_target,
            #[cfg(any(feature = "x11", feature = "wayland"))]
            _ => unreachable!(),
        };

        f(state)
    }

    fn loop_dispatch<D: Into<Option<std::time::Duration>>>(
        &mut self,
        timeout: D,
    ) -> std::io::Result<()> {
        let mut state = match &mut self.window_target.p {
            crate::platform_impl::EventLoopWindowTarget::Kms(window_target) => {
                &mut window_target.event_sink
            }
            #[cfg(any(feature = "x11", feature = "kms"))]
            _ => unreachable!(),
        };

        self.event_loop.dispatch(timeout, &mut state)
    }
}

/// A handle that can be sent across the threads and used to wake up the `EventLoop`.
pub struct EventLoopProxy<T: 'static> {
    user_events_sender: calloop::channel::Sender<T>,
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy {
            user_events_sender: self.user_events_sender.clone(),
        }
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn new(user_events_sender: calloop::channel::Sender<T>) -> Self {
        Self { user_events_sender }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.user_events_sender
            .send(event)
            .map_err(|SendError(error)| EventLoopClosed(error))
    }
}
