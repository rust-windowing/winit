use std::cell::{Cell, RefCell};
use std::clone::Clone;
use std::collections::{vec_deque::IntoIter as VecDequeIter, VecDeque};
use std::iter;
use std::rc::Rc;

use raw_window_handle::{RawDisplayHandle, WebDisplayHandle};

use super::{
    super::{monitor::MonitorHandle, KeyEventExtra},
    backend,
    device::DeviceId,
    proxy::EventLoopProxy,
    runner,
    window::WindowId,
};
use crate::dpi::{PhysicalSize, Size};
use crate::event::{
    DeviceEvent, DeviceId as RootDeviceId, ElementState, Event, KeyEvent, Touch, TouchPhase,
    WindowEvent,
};
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

    pub fn run(&self, event_handler: Box<runner::EventHandler<T>>) {
        self.runner.set_listener(event_handler);
        let runner = self.runner.clone();
        self.runner.set_on_scale_change(move |arg| {
            runner.handle_scale_changed(arg.old_scale, arg.new_scale)
        });
    }

    pub fn generate_id(&self) -> WindowId {
        WindowId(self.runner.generate_id())
    }

    pub fn register(
        &self,
        canvas: &Rc<RefCell<backend::Canvas>>,
        id: WindowId,
        prevent_default: bool,
        has_focus: Rc<Cell<bool>>,
    ) {
        self.runner.add_canvas(RootWindowId(id), canvas);
        let mut canvas = canvas.borrow_mut();
        canvas.set_attribute("data-raw-handle", &id.0.to_string());

        canvas.on_touch_start(prevent_default);
        canvas.on_touch_end(prevent_default);

        let runner = self.runner.clone();
        let has_focus_clone = has_focus.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_blur(move || {
            has_focus_clone.set(false);

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
        let has_focus_clone = has_focus.clone();
        canvas.on_focus(move || {
            if !has_focus_clone.replace(true) {
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

                runner.send_events(
                    iter::once(Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::KeyboardInput {
                            device_id: RootDeviceId(unsafe { DeviceId::dummy() }),
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
                    })
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

                runner.send_events(
                    iter::once(Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::KeyboardInput {
                            device_id: RootDeviceId(unsafe { DeviceId::dummy() }),
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
                    })
                    .chain(modifiers_changed),
                )
            },
            prevent_default,
        );

        let runner = self.runner.clone();
        let modifiers = self.modifiers.clone();
        let has_focus_clone = has_focus.clone();
        canvas.on_cursor_leave(move |pointer_id, active_modifiers| {
            let modifiers_changed = (has_focus_clone.get() && modifiers.get() != active_modifiers)
                .then(|| {
                    modifiers.set(active_modifiers);
                    Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                    }
                });

            runner.send_events(modifiers_changed.into_iter().chain(iter::once(
                Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::CursorLeft {
                        device_id: RootDeviceId(DeviceId(pointer_id)),
                    },
                },
            )));
        });

        let runner = self.runner.clone();
        let modifiers = self.modifiers.clone();
        let has_focus_clone = has_focus.clone();
        canvas.on_cursor_enter(move |pointer_id, active_modifiers| {
            let modifiers_changed = (has_focus_clone.get() && modifiers.get() != active_modifiers)
                .then(|| {
                    modifiers.set(active_modifiers);
                    Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                    }
                });

            runner.send_events(modifiers_changed.into_iter().chain(iter::once(
                Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::CursorEntered {
                        device_id: RootDeviceId(DeviceId(pointer_id)),
                    },
                },
            )));
        });

        canvas.on_cursor_move(
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers| {
                    if has_focus.get() && modifiers.get() != active_modifiers {
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

                move |pointer_id, position, delta| {
                    runner.send_events(
                        [
                            Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::CursorMoved {
                                    device_id: RootDeviceId(DeviceId(pointer_id)),
                                    position,
                                },
                            },
                            Event::DeviceEvent {
                                device_id: RootDeviceId(DeviceId(pointer_id)),
                                event: DeviceEvent::MouseMotion {
                                    delta: (delta.x, delta.y),
                                },
                            },
                        ]
                        .into_iter(),
                    );
                }
            },
            {
                let runner = self.runner.clone();

                move |device_id, location, force| {
                    runner.send_event(Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::Touch(Touch {
                            id: device_id as u64,
                            device_id: RootDeviceId(DeviceId(device_id)),
                            phase: TouchPhase::Moved,
                            force: Some(force),
                            location,
                        }),
                    });
                }
            },
            {
                let runner = self.runner.clone();

                move |pointer_id, position: crate::dpi::PhysicalPosition<f64>, buttons, button| {
                    let button_event = if buttons.contains(button.into()) {
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::MouseInput {
                                device_id: RootDeviceId(DeviceId(pointer_id)),
                                state: ElementState::Pressed,
                                button,
                            },
                        }
                    } else {
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::MouseInput {
                                device_id: RootDeviceId(DeviceId(pointer_id)),
                                state: ElementState::Released,
                                button,
                            },
                        }
                    };

                    // A chorded button event may come in without any prior CursorMoved events,
                    // therefore we should send a CursorMoved event to make sure that the
                    // user code has the correct cursor position.
                    runner.send_events(
                        [
                            Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::CursorMoved {
                                    device_id: RootDeviceId(DeviceId(pointer_id)),
                                    position,
                                },
                            },
                            button_event,
                        ]
                        .into_iter(),
                    );
                }
            },
            prevent_default,
        );

        canvas.on_mouse_press(
            {
                let runner = self.runner.clone();
                let modifiers = self.modifiers.clone();
                let has_focus = has_focus.clone();

                move |pointer_id, position, button, active_modifiers| {
                    let focus_changed =
                        (!has_focus.replace(true)).then_some(Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::Focused(true),
                        });

                    let modifiers_changed = (modifiers.get() != active_modifiers).then(|| {
                        modifiers.set(active_modifiers);
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        }
                    });

                    // A mouse down event may come in without any prior CursorMoved events,
                    // therefore we should send a CursorMoved event to make sure that the
                    // user code has the correct cursor position.
                    runner.send_events(focus_changed.into_iter().chain(modifiers_changed).chain([
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::CursorMoved {
                                device_id: RootDeviceId(DeviceId(pointer_id)),
                                position,
                            },
                        },
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::MouseInput {
                                device_id: RootDeviceId(DeviceId(pointer_id)),
                                state: ElementState::Pressed,
                                button,
                            },
                        },
                    ]));
                }
            },
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();

                move |device_id, location, force| {
                    let focus_changed =
                        (!has_focus.replace(true)).then_some(Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::Focused(true),
                        });

                    runner.send_events(focus_changed.into_iter().chain(iter::once(
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
                    )));
                }
            },
        );

        canvas.on_mouse_release(
            {
                let runner = self.runner.clone();
                let modifiers = self.modifiers.clone();
                let has_focus = has_focus.clone();

                move |pointer_id, position, button, active_modifiers| {
                    let modifiers_changed =
                        (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
                            modifiers.set(active_modifiers);
                            Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                            }
                        });

                    // A mouse up event may come in without any prior CursorMoved events,
                    // therefore we should send a CursorMoved event to make sure that the
                    // user code has the correct cursor position.
                    runner.send_events(modifiers_changed.into_iter().chain([
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::CursorMoved {
                                device_id: RootDeviceId(DeviceId(pointer_id)),
                                position,
                            },
                        },
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::MouseInput {
                                device_id: RootDeviceId(DeviceId(pointer_id)),
                                state: ElementState::Released,
                                button,
                            },
                        },
                    ]));
                }
            },
            {
                let runner_touch = self.runner.clone();

                move |device_id, location, force| {
                    runner_touch.send_event(Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::Touch(Touch {
                            id: device_id as u64,
                            device_id: RootDeviceId(DeviceId(device_id)),
                            phase: TouchPhase::Ended,
                            force: Some(force),
                            location,
                        }),
                    });
                }
            },
        );

        let runner = self.runner.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_mouse_wheel(
            move |pointer_id, delta, active_modifiers| {
                let modifiers_changed = (has_focus.get() && modifiers.get() != active_modifiers)
                    .then(|| {
                        modifiers.set(active_modifiers);
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        }
                    });

                runner.send_events(modifiers_changed.into_iter().chain(iter::once(
                    Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::MouseWheel {
                            device_id: RootDeviceId(DeviceId(pointer_id)),
                            delta,
                            phase: TouchPhase::Moved,
                        },
                    },
                )));
            },
            prevent_default,
        );

        let runner = self.runner.clone();
        let raw = canvas.raw().clone();

        // The size to restore to after exiting fullscreen.
        let mut intended_size = PhysicalSize {
            width: raw.width(),
            height: raw.height(),
        };
        canvas.on_fullscreen_change(move || {
            // If the canvas is marked as fullscreen, it is moving *into* fullscreen
            // If it is not, it is moving *out of* fullscreen
            let new_size = if backend::is_fullscreen(&raw) {
                intended_size = PhysicalSize {
                    width: raw.width(),
                    height: raw.height(),
                };

                backend::window_size().to_physical(backend::scale_factor())
            } else {
                intended_size
            };

            backend::set_canvas_size(&raw, Size::Physical(new_size));
            runner.send_event(Event::WindowEvent {
                window_id: RootWindowId(id),
                event: WindowEvent::Resized(new_size),
            });
            runner.request_redraw(RootWindowId(id));
        });

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
}
