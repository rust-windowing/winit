use super::{
    super::{monitor, KeyEventExtra},
    backend, device,
    proxy::Proxy,
    runner, window,
};
use crate::dpi::{PhysicalSize, Size};
use crate::event::{DeviceEvent, DeviceId, ElementState, Event, KeyEvent, TouchPhase, WindowEvent};
use crate::event_loop::ControlFlow;
use crate::keyboard::ModifiersState;
use crate::monitor::MonitorHandle as RootMH;
use crate::window::{Theme, WindowId};
use std::cell::{Cell, RefCell};
use std::clone::Clone;
use std::collections::{vec_deque::IntoIter as VecDequeIter, VecDeque};
use std::rc::Rc;

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

pub struct WindowTarget<T: 'static> {
    pub(crate) runner: runner::Shared<T>,
    modifiers: ModifiersShared,
}

impl<T> Clone for WindowTarget<T> {
    fn clone(&self) -> Self {
        WindowTarget {
            runner: self.runner.clone(),
            modifiers: self.modifiers.clone(),
        }
    }
}

impl<T> WindowTarget<T> {
    pub fn new() -> Self {
        WindowTarget {
            runner: runner::Shared::new(),
            modifiers: ModifiersShared::default(),
        }
    }

    pub fn proxy(&self) -> Proxy<T> {
        Proxy::new(self.runner.clone())
    }

    pub fn run(&self, event_handler: Box<dyn FnMut(Event<'_, T>, &mut ControlFlow)>) {
        self.runner.set_listener(event_handler);
        let runner = self.runner.clone();
        self.runner.set_on_scale_change(move |arg| {
            runner.handle_scale_changed(arg.old_scale, arg.new_scale)
        });
    }

    pub fn generate_id(&self) -> window::Id {
        window::Id(self.runner.generate_id())
    }

    pub fn register(&self, canvas: &Rc<RefCell<backend::Canvas>>, id: window::Id) {
        self.runner.add_canvas(WindowId(id), canvas);
        let mut canvas = canvas.borrow_mut();
        canvas.set_attribute("data-raw-handle", &id.0.to_string());

        let runner = self.runner.clone();
        canvas.on_blur(move || {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::Focused(false),
            });
        });

        let runner = self.runner.clone();
        canvas.on_focus(move || {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::Focused(true),
            });
        });

        let runner = self.runner.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_keyboard_press(
            move |physical_key, logical_key, text, location, repeat, new_modifiers| {
                let active_modifiers = modifiers.get() | new_modifiers;
                let modifiers_changed = if modifiers.get() != active_modifiers {
                    modifiers.set(active_modifiers);
                    Some(Event::WindowEvent {
                        window_id: WindowId(id),
                        event: WindowEvent::ModifiersChanged(active_modifiers),
                    })
                } else {
                    None
                };

                runner.send_events(
                    std::iter::once(Event::WindowEvent {
                        window_id: WindowId(id),
                        event: WindowEvent::KeyboardInput {
                            device_id: DeviceId(unsafe { device::Id::dummy() }),
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
        );

        let runner = self.runner.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_keyboard_release(
            move |physical_key, logical_key, text, location, repeat, new_modifiers| {
                let active_modifiers = modifiers.get() & !new_modifiers;
                let modifiers_changed = if modifiers.get() != active_modifiers {
                    modifiers.set(active_modifiers);
                    Some(Event::WindowEvent {
                        window_id: WindowId(id),
                        event: WindowEvent::ModifiersChanged(active_modifiers),
                    })
                } else {
                    None
                };

                runner.send_events(
                    std::iter::once(Event::WindowEvent {
                        window_id: WindowId(id),
                        event: WindowEvent::KeyboardInput {
                            device_id: DeviceId(unsafe { device::Id::dummy() }),
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
        );

        let runner = self.runner.clone();
        canvas.on_composition_end(move |data| {
            if let Some(data) = data {
                runner.send_event(Event::WindowEvent {
                    window_id: WindowId(id),
                    event: WindowEvent::ReceivedImeText(data),
                });
            }
        });

        let runner = self.runner.clone();
        canvas.on_cursor_leave(move |pointer_id| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::CursorLeft {
                    device_id: DeviceId(device::Id(pointer_id)),
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_cursor_enter(move |pointer_id| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::CursorEntered {
                    device_id: DeviceId(device::Id(pointer_id)),
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_cursor_move(move |pointer_id, position, delta, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::CursorMoved {
                    device_id: DeviceId(device::Id(pointer_id)),
                    position,
                    modifiers,
                },
            });
            runner.send_event(Event::DeviceEvent {
                device_id: DeviceId(device::Id(pointer_id)),
                event: DeviceEvent::MouseMotion {
                    delta: (delta.x, delta.y),
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_mouse_press(move |pointer_id, position, button, modifiers| {
            // A mouse down event may come in without any prior CursorMoved events,
            // therefore we should send a CursorMoved event to make sure that the
            // user code has the correct cursor position.
            runner.send_events(
                std::iter::once(Event::WindowEvent {
                    window_id: WindowId(id),
                    event: WindowEvent::CursorMoved {
                        device_id: DeviceId(device::Id(pointer_id)),
                        position,
                        modifiers,
                    },
                })
                .chain(std::iter::once(Event::WindowEvent {
                    window_id: WindowId(id),
                    event: WindowEvent::MouseInput {
                        device_id: DeviceId(device::Id(pointer_id)),
                        state: ElementState::Pressed,
                        button,
                        modifiers,
                    },
                })),
            );
        });

        let runner = self.runner.clone();
        canvas.on_mouse_release(move |pointer_id, button, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::MouseInput {
                    device_id: DeviceId(device::Id(pointer_id)),
                    state: ElementState::Released,
                    button,
                    modifiers,
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_mouse_wheel(move |pointer_id, delta, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::MouseWheel {
                    device_id: DeviceId(device::Id(pointer_id)),
                    delta,
                    phase: TouchPhase::Moved,
                    modifiers,
                },
            });
        });

        let runner = self.runner.clone();
        let raw = canvas.raw().clone();

        // The size to restore to after exiting fullscreen.
        let mut intended_size = PhysicalSize {
            width: raw.width() as u32,
            height: raw.height() as u32,
        };
        canvas.on_fullscreen_change(move || {
            // If the canvas is marked as fullscreen, it is moving *into* fullscreen
            // If it is not, it is moving *out of* fullscreen
            let new_size = if backend::is_fullscreen(&raw) {
                intended_size = PhysicalSize {
                    width: raw.width() as u32,
                    height: raw.height() as u32,
                };

                backend::window_size().to_physical(backend::scale_factor())
            } else {
                intended_size
            };

            backend::set_canvas_size(&raw, Size::Physical(new_size));
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::Resized(new_size),
            });
            runner.request_redraw(WindowId(id));
        });

        let runner = self.runner.clone();
        canvas.on_dark_mode(move |is_dark_mode| {
            let theme = if is_dark_mode {
                Theme::Dark
            } else {
                Theme::Light
            };
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::ThemeChanged(theme),
            });
        });
    }

    pub fn available_monitors(&self) -> VecDequeIter<monitor::Handle> {
        VecDeque::new().into_iter()
    }

    pub fn primary_monitor(&self) -> Option<RootMH> {
        Some(RootMH {
            inner: monitor::Handle,
        })
    }
}
