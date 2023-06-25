use std::cell::{Cell, RefCell};
use std::clone::Clone;
use std::collections::{vec_deque::IntoIter as VecDequeIter, VecDeque};
use std::iter;
use std::rc::Rc;
use std::sync::atomic::Ordering;

use raw_window_handle::{RawDisplayHandle, WebDisplayHandle};

use super::runner::EventWrapper;
use super::{
    super::{monitor::MonitorHandle, KeyEventExtra},
    backend,
    device::DeviceId,
    proxy::EventLoopProxy,
    runner,
    window::WindowId,
};
use crate::event::{
    DeviceEvent, DeviceId as RootDeviceId, ElementState, Event, KeyEvent, RawKeyEvent, Touch,
    TouchPhase, WindowEvent,
};
use crate::event_loop::DeviceEvents;
use crate::keyboard::ModifiersState;
use crate::window::{Theme, WindowId as RootWindowId};

#[derive(Default)]
struct ModifiersShared(Rc<Cell<ModifiersState>>);

impl ModifiersShared {
    fn set(&self, new: ModifiersState) {
        self.0.set(new)
    }

    fn get(&self) -> ModifiersState {
        self.0.get()
    }
}

impl Clone for ModifiersShared {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

pub struct EventLoopWindowTarget<T: 'static> {
    pub(crate) runner: runner::Shared<T>,
    modifiers: ModifiersShared,
}

impl<T> Clone for EventLoopWindowTarget<T> {
    fn clone(&self) -> Self {
        Self {
            runner: self.runner.clone(),
            modifiers: self.modifiers.clone(),
        }
    }
}

impl<T> EventLoopWindowTarget<T> {
    pub fn new() -> Self {
        Self {
            runner: runner::Shared::new(),
            modifiers: ModifiersShared::default(),
        }
    }

    pub fn proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::new(self.runner.clone())
    }

    pub fn run(&self, event_handler: Box<runner::EventHandler<T>>, event_loop_recreation: bool) {
        self.runner.event_loop_recreation(event_loop_recreation);
        self.runner.set_listener(event_handler);
    }

    pub fn generate_id(&self) -> WindowId {
        WindowId(self.runner.generate_id())
    }

    pub fn register(
        &self,
        canvas: &Rc<RefCell<backend::Canvas>>,
        id: WindowId,
        prevent_default: bool,
    ) {
        self.runner.add_canvas(RootWindowId(id), canvas);
        let canvas_clone = canvas.clone();
        let mut canvas = canvas.borrow_mut();
        canvas.set_attribute("data-raw-handle", &id.0.to_string());

        canvas.on_touch_start(prevent_default);

        let runner = self.runner.clone();
        let has_focus = canvas.has_focus.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_blur(move || {
            has_focus.store(false, Ordering::Relaxed);

            let clear_modifiers = (!modifiers.get().is_empty()).then(|| {
                modifiers.set(ModifiersState::empty());
                Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::ModifiersChanged(ModifiersState::empty().into()),
                }
            });

            runner.send_events(
                clear_modifiers
                    .into_iter()
                    .chain(iter::once(Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::Focused(false),
                    })),
            );
        });

        let runner = self.runner.clone();
        let has_focus = canvas.has_focus.clone();
        canvas.on_focus(move || {
            if !has_focus.swap(true, Ordering::Relaxed) {
                runner.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::Focused(true),
                });
            }
        });

        let runner = self.runner.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_keyboard_press(
            move |physical_key, logical_key, text, location, repeat, active_modifiers| {
                let modifiers_changed = (modifiers.get() != active_modifiers).then(|| {
                    modifiers.set(active_modifiers);
                    Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                    }
                });

                let device_id = RootDeviceId(unsafe { DeviceId::dummy() });

                let device_event = runner.device_events().then_some(Event::DeviceEvent {
                    device_id,
                    event: DeviceEvent::Key(RawKeyEvent {
                        physical_key,
                        state: ElementState::Pressed,
                    }),
                });

                runner.send_events(
                    device_event
                        .into_iter()
                        .chain(iter::once(Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::KeyboardInput {
                                device_id,
                                event: KeyEvent {
                                    physical_key,
                                    logical_key,
                                    text,
                                    location,
                                    state: ElementState::Pressed,
                                    repeat,
                                    platform_specific: KeyEventExtra,
                                },
                                is_synthetic: false,
                            },
                        }))
                        .chain(modifiers_changed),
                );
            },
            prevent_default,
        );

        let runner = self.runner.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_keyboard_release(
            move |physical_key, logical_key, text, location, repeat, active_modifiers| {
                let modifiers_changed = (modifiers.get() != active_modifiers).then(|| {
                    modifiers.set(active_modifiers);
                    Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                    }
                });

                let device_id = RootDeviceId(unsafe { DeviceId::dummy() });

                let device_event = runner.device_events().then_some(Event::DeviceEvent {
                    device_id,
                    event: DeviceEvent::Key(RawKeyEvent {
                        physical_key,
                        state: ElementState::Pressed,
                    }),
                });

                runner.send_events(
                    device_event
                        .into_iter()
                        .chain(iter::once(Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::KeyboardInput {
                                device_id,
                                event: KeyEvent {
                                    physical_key,
                                    logical_key,
                                    text,
                                    location,
                                    state: ElementState::Released,
                                    repeat,
                                    platform_specific: KeyEventExtra,
                                },
                                is_synthetic: false,
                            },
                        }))
                        .chain(modifiers_changed),
                )
            },
            prevent_default,
        );

        let has_focus = canvas.has_focus.clone();
        canvas.on_cursor_leave({
            let runner = self.runner.clone();
            let has_focus = has_focus.clone();
            let modifiers = self.modifiers.clone();

            move |active_modifiers, pointer_id| {
                let focus = (has_focus.load(Ordering::Relaxed)
                    && modifiers.get() != active_modifiers)
                    .then(|| {
                        modifiers.set(active_modifiers);
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        }
                    });

                let pointer = pointer_id.map(|pointer_id| Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::CursorLeft {
                        device_id: RootDeviceId(DeviceId(pointer_id)),
                    },
                });

                if focus.is_some() || pointer.is_some() {
                    runner.send_events(focus.into_iter().chain(pointer))
                }
            }
        });

        canvas.on_cursor_enter({
            let runner = self.runner.clone();
            let has_focus = has_focus.clone();
            let modifiers = self.modifiers.clone();

            move |active_modifiers, pointer_id| {
                let focus = (has_focus.load(Ordering::Relaxed)
                    && modifiers.get() != active_modifiers)
                    .then(|| {
                        modifiers.set(active_modifiers);
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        }
                    });

                let pointer = pointer_id.map(|pointer_id| Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::CursorEntered {
                        device_id: RootDeviceId(DeviceId(pointer_id)),
                    },
                });

                if focus.is_some() || pointer.is_some() {
                    runner.send_events(focus.into_iter().chain(pointer))
                }
            }
        });

        canvas.on_cursor_move(
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers| {
                    if has_focus.load(Ordering::Relaxed) && modifiers.get() != active_modifiers {
                        modifiers.set(active_modifiers);
                        runner.send_event(Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        })
                    }
                }
            },
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers, pointer_id, events| {
                    let modifiers = (has_focus.load(Ordering::Relaxed)
                        && modifiers.get() != active_modifiers)
                        .then(|| {
                            modifiers.set(active_modifiers);
                            Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                            }
                        });

                    runner.send_events(modifiers.into_iter().chain(events.flat_map(
                        |(position, delta)| {
                            let device_id = RootDeviceId(DeviceId(pointer_id));

                            let device_events = runner.device_events().then(|| {
                                let x_motion = (delta.x != 0.0).then_some(Event::DeviceEvent {
                                    device_id,
                                    event: DeviceEvent::Motion {
                                        axis: 0,
                                        value: delta.x,
                                    },
                                });

                                let y_motion = (delta.y != 0.0).then_some(Event::DeviceEvent {
                                    device_id,
                                    event: DeviceEvent::Motion {
                                        axis: 1,
                                        value: delta.y,
                                    },
                                });

                                x_motion.into_iter().chain(y_motion).chain(iter::once(
                                    Event::DeviceEvent {
                                        device_id,
                                        event: DeviceEvent::MouseMotion {
                                            delta: (delta.x, delta.y),
                                        },
                                    },
                                ))
                            });

                            device_events.into_iter().flatten().chain(iter::once(
                                Event::WindowEvent {
                                    window_id: RootWindowId(id),
                                    event: WindowEvent::CursorMoved {
                                        device_id,
                                        position,
                                    },
                                },
                            ))
                        },
                    )));
                }
            },
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers, device_id, events| {
                    let modifiers = (has_focus.load(Ordering::Relaxed)
                        && modifiers.get() != active_modifiers)
                        .then(|| {
                            modifiers.set(active_modifiers);
                            Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                            }
                        });

                    runner.send_events(modifiers.into_iter().chain(events.map(
                        |(location, force)| Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::Touch(Touch {
                                id: device_id as u64,
                                device_id: RootDeviceId(DeviceId(device_id)),
                                phase: TouchPhase::Moved,
                                force: Some(force),
                                location,
                            }),
                        },
                    )));
                }
            },
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers,
                      pointer_id,
                      position: crate::dpi::PhysicalPosition<f64>,
                      buttons,
                      button| {
                    let modifiers = (has_focus.load(Ordering::Relaxed)
                        && modifiers.get() != active_modifiers)
                        .then(|| {
                            modifiers.set(active_modifiers);
                            Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                            }
                        });

                    let device_id = RootDeviceId(DeviceId(pointer_id));

                    let state = if buttons.contains(button.into()) {
                        ElementState::Pressed
                    } else {
                        ElementState::Released
                    };

                    let device_event = runner.device_events().then(|| Event::DeviceEvent {
                        device_id,
                        event: DeviceEvent::Button {
                            button: button.to_id(),
                            state,
                        },
                    });

                    // A chorded button event may come in without any prior CursorMoved events,
                    // therefore we should send a CursorMoved event to make sure that the
                    // user code has the correct cursor position.
                    runner.send_events(modifiers.into_iter().chain(device_event).chain([
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::CursorMoved {
                                device_id,
                                position,
                            },
                        },
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::MouseInput {
                                device_id,
                                state,
                                button,
                            },
                        },
                    ]));
                }
            },
            prevent_default,
        );

        canvas.on_mouse_press(
            {
                let runner = self.runner.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers| {
                    if modifiers.get() != active_modifiers {
                        modifiers.set(active_modifiers);
                        runner.send_event(Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        })
                    }
                }
            },
            {
                let runner = self.runner.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers, pointer_id, position, button| {
                    let modifiers = (modifiers.get() != active_modifiers).then(|| {
                        modifiers.set(active_modifiers);
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        }
                    });

                    let device_id: RootDeviceId = RootDeviceId(DeviceId(pointer_id));
                    let device_event = runner.device_events().then(|| Event::DeviceEvent {
                        device_id,
                        event: DeviceEvent::Button {
                            button: button.to_id(),
                            state: ElementState::Pressed,
                        },
                    });

                    // A mouse down event may come in without any prior CursorMoved events,
                    // therefore we should send a CursorMoved event to make sure that the
                    // user code has the correct cursor position.
                    runner.send_events(modifiers.into_iter().chain(device_event).chain([
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::CursorMoved {
                                device_id,
                                position,
                            },
                        },
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::MouseInput {
                                device_id,
                                state: ElementState::Pressed,
                                button,
                            },
                        },
                    ]));
                }
            },
            {
                let runner = self.runner.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers, device_id, location, force| {
                    let modifiers = (modifiers.get() != active_modifiers).then(|| {
                        modifiers.set(active_modifiers);
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        }
                    });

                    runner.send_events(modifiers.into_iter().chain(iter::once(
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::Touch(Touch {
                                id: device_id as u64,
                                device_id: RootDeviceId(DeviceId(device_id)),
                                phase: TouchPhase::Started,
                                force: Some(force),
                                location,
                            }),
                        },
                    )))
                }
            },
            prevent_default,
        );

        canvas.on_mouse_release(
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers| {
                    if has_focus.load(Ordering::Relaxed) && modifiers.get() != active_modifiers {
                        modifiers.set(active_modifiers);
                        runner.send_event(Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        });
                    }
                }
            },
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers, pointer_id, position, button| {
                    let modifiers = (has_focus.load(Ordering::Relaxed)
                        && modifiers.get() != active_modifiers)
                        .then(|| {
                            modifiers.set(active_modifiers);
                            Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                            }
                        });

                    let device_id: RootDeviceId = RootDeviceId(DeviceId(pointer_id));
                    let device_event = runner.device_events().then(|| Event::DeviceEvent {
                        device_id,
                        event: DeviceEvent::Button {
                            button: button.to_id(),
                            state: ElementState::Pressed,
                        },
                    });

                    // A mouse up event may come in without any prior CursorMoved events,
                    // therefore we should send a CursorMoved event to make sure that the
                    // user code has the correct cursor position.
                    runner.send_events(modifiers.into_iter().chain(device_event).chain([
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::CursorMoved {
                                device_id,
                                position,
                            },
                        },
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::MouseInput {
                                device_id,
                                state: ElementState::Released,
                                button,
                            },
                        },
                    ]));
                }
            },
            {
                let runner_touch = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers, device_id, location, force| {
                    let modifiers = (has_focus.load(Ordering::Relaxed)
                        && modifiers.get() != active_modifiers)
                        .then(|| {
                            modifiers.set(active_modifiers);
                            Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                            }
                        });

                    runner_touch.send_events(modifiers.into_iter().chain(iter::once(
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::Touch(Touch {
                                id: device_id as u64,
                                device_id: RootDeviceId(DeviceId(device_id)),
                                phase: TouchPhase::Ended,
                                force: Some(force),
                                location,
                            }),
                        },
                    )));
                }
            },
        );

        let runner = self.runner.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_mouse_wheel(
            move |pointer_id, delta, active_modifiers| {
                let modifiers_changed = (has_focus.load(Ordering::Relaxed)
                    && modifiers.get() != active_modifiers)
                    .then(|| {
                        modifiers.set(active_modifiers);
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        }
                    });

                let device_event = runner.device_events().then_some(Event::DeviceEvent {
                    device_id: RootDeviceId(DeviceId(pointer_id)),
                    event: DeviceEvent::MouseWheel { delta },
                });

                runner.send_events(modifiers_changed.into_iter().chain(device_event).chain(
                    iter::once(Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::MouseWheel {
                            device_id: RootDeviceId(DeviceId(pointer_id)),
                            delta,
                            phase: TouchPhase::Moved,
                        },
                    }),
                ));
            },
            prevent_default,
        );

        let runner = self.runner.clone();
        canvas.on_touch_cancel(move |device_id, location, force| {
            runner.send_event(Event::WindowEvent {
                window_id: RootWindowId(id),
                event: WindowEvent::Touch(Touch {
                    id: device_id as u64,
                    device_id: RootDeviceId(DeviceId(device_id)),
                    phase: TouchPhase::Cancelled,
                    force: Some(force),
                    location,
                }),
            });
        });

        let runner = self.runner.clone();
        canvas.on_dark_mode(move |is_dark_mode| {
            let theme = if is_dark_mode {
                Theme::Dark
            } else {
                Theme::Light
            };
            runner.send_event(Event::WindowEvent {
                window_id: RootWindowId(id),
                event: WindowEvent::ThemeChanged(theme),
            });
        });

        canvas.on_resize_scale(
            {
                let runner = self.runner.clone();
                let canvas = canvas_clone.clone();

                move |size, scale| {
                    runner.send_event(EventWrapper::ScaleChange {
                        canvas: Rc::downgrade(&canvas),
                        size,
                        scale,
                    })
                }
            },
            {
                let runner = self.runner.clone();

                move |new_size| {
                    let canvas = RefCell::borrow(&canvas_clone);
                    canvas.set_current_size(new_size);
                    if canvas.old_size() != new_size {
                        canvas.set_old_size(new_size);
                        runner.send_event(Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::Resized(new_size),
                        });
                        runner.request_redraw(RootWindowId(id));
                    }
                }
            },
        );
    }

    pub fn available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle)
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Web(WebDisplayHandle::empty())
    }

    pub fn listen_device_events(&self, allowed: DeviceEvents) {
        self.runner.listen_device_events(allowed)
    }
}
