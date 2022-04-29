use std::{
    collections::VecDeque,
    marker::PhantomData,
    os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd, OpenOptionsExt, RawFd},
    path::Path,
    sync::mpsc::SendError,
};

use calloop::{EventSource, Interest, Mode, Poll, PostAction, Readiness, Token, TokenFactory};
use drm::control::{property, Device, ResourceHandle};
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
use instant::{Duration, Instant};
use libc::{O_RDONLY, O_RDWR, O_WRONLY};

use crate::{
    dpi::PhysicalPosition,
    event::{Force, KeyboardInput, ModifiersState, MouseScrollDelta, StartCause},
    event_loop::{ControlFlow, EventLoopClosed},
    platform_impl::{platform::sticky_exit_callback, GBM_DEVICE},
};

use super::input_to_vk::CHAR_MAPPINGS;

struct Interface;

impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<RawFd, i32> {
        std::fs::OpenOptions::new()
            .custom_flags(flags)
            .read((flags & O_RDONLY != 0) | (flags & O_RDWR != 0))
            .write((flags & O_WRONLY != 0) | (flags & O_RDWR != 0))
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

#[derive(Debug)]
pub struct LibinputInputBackend {
    context: input::Libinput,
    token: Token,
    touch_location: PhysicalPosition<f64>,
    screen_size: (u32, u32),
    modifiers: ModifiersState,
    cursor_positon: PhysicalPosition<f64>,
}

impl LibinputInputBackend {
    /// Initialize a new [`LibinputInputBackend`] from a given already initialized
    /// [libinput context](libinput::Libinput).
    pub fn new(context: input::Libinput, screen_size: (u32, u32)) -> Self {
        LibinputInputBackend {
            context,
            token: Token::invalid(),
            touch_location: PhysicalPosition::new(0.0, 0.0),
            cursor_positon: PhysicalPosition::new(0.0, 0.0),
            modifiers: ModifiersState::empty(),
            screen_size,
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
                                        crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                    crate::platform_impl::WindowId::Drm(super::WindowId),
                                ),
                                event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::WindowId::Drm(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::WindowId::Drm(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                    crate::platform_impl::WindowId::Drm(super::WindowId),
                                ),
                                event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                    crate::platform_impl::WindowId::Drm(super::WindowId),
                                ),
                                event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                    crate::platform_impl::WindowId::Drm(super::WindowId),
                                ),
                                event: crate::event::WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::WindowId::Drm(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::MouseInput {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::WindowId::Drm(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::CursorMoved {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::WindowId::Drm(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::MouseInput {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::WindowId::Drm(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::MouseWheel {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::WindowId::Drm(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::MouseWheel {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                                        crate::platform_impl::WindowId::Drm(super::WindowId),
                                    ),
                                    event: crate::event::WindowEvent::CursorMoved {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Drm(super::DeviceId),
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
                    input::Event::Keyboard(ev) => match &ev {
                        input::event::KeyboardEvent::Key(key) => match key.key() {
                                56 // LAlt
                                    | 100 // RAlt
                                    => {
                                        match key.key_state() {
                                            KeyState::Pressed => self.modifiers |= ModifiersState::ALT,
                                            KeyState::Released => self.modifiers.remove(ModifiersState::ALT)
                                        }
                                        callback(crate::event::Event::WindowEvent {
                                            window_id: crate::window::WindowId(crate::platform_impl::WindowId::Drm(super::WindowId)),
                                            event:crate::event::WindowEvent::ModifiersChanged(self.modifiers)}, &mut ());
                                    }
                                | 42 // LShift
                                    | 54 // RShift
                                    => {
                                        match key.key_state() {
                                            KeyState::Pressed => self.modifiers |= ModifiersState::SHIFT,
                                            KeyState::Released => self.modifiers.remove(ModifiersState::SHIFT)
                                        }
                                        callback(crate::event::Event::WindowEvent {
                                            window_id: crate::window::WindowId(crate::platform_impl::WindowId::Drm(super::WindowId)),
                                            event:crate::event::WindowEvent::ModifiersChanged(self.modifiers)}, &mut ());
                                    }

                                | 29 // LCtrl
                                    | 97 // RCtrl
                                    => {
                                        match key.key_state() {
                                            KeyState::Pressed => self.modifiers |= ModifiersState::CTRL,
                                            KeyState::Released => self.modifiers.remove(ModifiersState::CTRL)
                                        }
                                        callback(crate::event::Event::WindowEvent {
                                            window_id: crate::window::WindowId(crate::platform_impl::WindowId::Drm(super::WindowId)),
                                            event:crate::event::WindowEvent::ModifiersChanged(self.modifiers)}, &mut ());
                                    }

                                | 125 // LMeta
                                    | 126 // RMeta
                                    => {
                                        match key.key_state() {
                                            KeyState::Pressed => self.modifiers |= ModifiersState::LOGO,
                                            KeyState::Released => self.modifiers.remove(ModifiersState::LOGO)
                                        }
                                        callback(crate::event::Event::WindowEvent {
                                            window_id: crate::window::WindowId(crate::platform_impl::WindowId::Drm(super::WindowId)),
                                            event:crate::event::WindowEvent::ModifiersChanged(self.modifiers)}, &mut ());
                                    }

                                k => {
                                    callback(crate::event::Event::WindowEvent {
                                        window_id: crate::window::WindowId(crate::platform_impl::WindowId::Drm(super::WindowId)),
                                        event: crate::event::WindowEvent::KeyboardInput { device_id: crate::event::DeviceId(crate::  platform_impl::DeviceId::Drm( super::DeviceId)),
                                        input: KeyboardInput { scancode: k, state: match ev.key_state() {
                                            KeyState::Pressed => crate::event::ElementState::Pressed,
                                            KeyState::Released => crate::event::ElementState::Released
                                        }, virtual_keycode: CHAR_MAPPINGS[k as usize], modifiers: self.modifiers } , is_synthetic: false }}, &mut ());
                                }
                            },
                        _ => {}
                    },
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
    /// gbm Connector
    pub connector: drm::control::connector::Info,

    /// gbm crtc
    pub crtc: drm::control::crtc::Info,

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
            inner: crate::platform_impl::MonitorHandle::Drm(super::MonitorHandle(
                self.connector.clone(),
            )),
        })
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<super::MonitorHandle> {
        if let Ok(gbm) = &**GBM_DEVICE.lock() {
            gbm.resource_handles()
                .unwrap()
                .connectors()
                .iter()
                .map(|f| super::MonitorHandle(gbm.get_connector(*f).unwrap()))
                .collect()
        } else {
            VecDeque::new()
        }
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
    card: &std::sync::Arc<gbm::Device<super::Card>>,
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
        match GBM_DEVICE.lock().as_ref() {
            Ok(gbm) => {
                drm::Device::set_client_capability(
                    gbm.as_ref(),
                    drm::ClientCapability::UniversalPlanes,
                    true,
                )
                .or(Err(crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::DrmMisc(
                        "kmsdrm device does not support universal planes",
                    ),
                )))?;
                drm::Device::set_client_capability(
                    gbm.as_ref(),
                    drm::ClientCapability::Atomic,
                    true,
                )
                .or(Err(crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::DrmMisc(
                        "kmsdrm device does not support atomic modesetting",
                    ),
                )))?;

                // Load the information.
                let res = gbm.resource_handles().or(Err(crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::DrmMisc("Could not load normal resource ids."),
                )))?;
                let coninfo: Vec<drm::control::connector::Info> = res
                    .connectors()
                    .iter()
                    .flat_map(|con| gbm.get_connector(*con))
                    .collect();
                let crtcinfo: Vec<drm::control::crtc::Info> = res
                    .crtcs()
                    .iter()
                    .flat_map(|crtc| gbm.get_crtc(*crtc))
                    .collect();

                let crtc = crtcinfo.get(0).ok_or(crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::DrmMisc("No crtcs found"),
                ))?;

                // Filter each connector until we find one that's connected.
                let con = coninfo
                    .iter()
                    .find(|&i| i.state() == drm::control::connector::State::Connected)
                    .ok_or(crate::error::OsError::new(
                        line!(),
                        file!(),
                        crate::platform_impl::OsError::DrmMisc("No connected connectors"),
                    ))?;

                // Get the first (usually best) mode
                let &mode = con.modes().get(0).ok_or(crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::DrmMisc("No modes found on connector"),
                ))?;

                let (disp_width, disp_height) = mode.size();

                let mut input = input::Libinput::new_with_udev(Interface);
                input.udev_assign_seat("seat0").unwrap();
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
                let (event_loop_awakener, event_loop_awakener_source) =
                    calloop::ping::make_ping().unwrap();

                let event_sink = EventSink::new();

                // Handler of window requests.
                handle
                    .insert_source(
                        event_loop_awakener_source,
                        move |_event, _metadata, data| {
                            data.push(crate::event::Event::RedrawRequested(
                                crate::window::WindowId(crate::platform_impl::WindowId::Drm(
                                    super::WindowId,
                                )),
                            ));
                        },
                    )
                    .unwrap();

                let input_backend: LibinputInputBackend =
                    LibinputInputBackend::new(input, (disp_width.into(), disp_height.into()));

                let input_loop: calloop::Dispatcher<'static, LibinputInputBackend, EventSink> =
                    calloop::Dispatcher::new(
                        input_backend,
                        move |event, _metadata, data: &mut EventSink| {
                            data.push(event);
                        },
                    );

                handle.register_dispatcher(input_loop).unwrap();

                let window_target = crate::event_loop::EventLoopWindowTarget {
                    p: crate::platform_impl::EventLoopWindowTarget::Drm(EventLoopWindowTarget {
                        connector: con.clone(),
                        crtc: crtc.clone(),
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
            Err(_) => Err(crate::error::OsError::new(
                line!(),
                file!(),
                crate::platform_impl::OsError::DrmMisc("gbm failed to initialize"),
            )),
        }
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
                crate::platform_impl::WindowId::Drm(super::WindowId),
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
            crate::platform_impl::EventLoopWindowTarget::Drm(window_target) => window_target,
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
            crate::platform_impl::EventLoopWindowTarget::Drm(window_target) => {
                &mut window_target.event_sink
            }
            #[cfg(any(feature = "x11", feature = "kmsdrm"))]
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
