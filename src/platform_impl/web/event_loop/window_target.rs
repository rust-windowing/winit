use super::{backend, device, proxy::Proxy, runner, window};
use crate::event::{DeviceId, ElementState, Event, KeyboardInput, TouchPhase, WindowEvent};
use crate::event_loop::ControlFlow;
use crate::window::WindowId;
use std::clone::Clone;

pub struct WindowTarget<T: 'static> {
    pub(crate) runner: runner::Shared<T>,
}

impl<T> Clone for WindowTarget<T> {
    fn clone(&self) -> Self {
        WindowTarget {
            runner: self.runner.clone(),
        }
    }
}

impl<T> WindowTarget<T> {
    pub fn new() -> Self {
        WindowTarget {
            runner: runner::Shared::new(),
        }
    }

    pub fn proxy(&self) -> Proxy<T> {
        Proxy::new(self.runner.clone())
    }

    pub fn run(&self, event_handler: Box<dyn FnMut(Event<T>, &mut ControlFlow)>) {
        self.runner.set_listener(event_handler);
    }

    pub fn register(&self, canvas: &mut backend::Canvas) {
        let runner = self.runner.clone();
        canvas.on_blur(move || {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::Focused(false),
            });
        });

        let runner = self.runner.clone();
        canvas.on_focus(move || {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::Focused(true),
            });
        });

        let runner = self.runner.clone();
        canvas.on_keyboard_press(move |scancode, virtual_keycode, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::KeyboardInput {
                    device_id: DeviceId(unsafe { device::Id::dummy() }),
                    input: KeyboardInput {
                        scancode,
                        state: ElementState::Pressed,
                        virtual_keycode,
                        modifiers,
                    },
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_keyboard_release(move |scancode, virtual_keycode, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::KeyboardInput {
                    device_id: DeviceId(unsafe { device::Id::dummy() }),
                    input: KeyboardInput {
                        scancode,
                        state: ElementState::Released,
                        virtual_keycode,
                        modifiers,
                    },
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_received_character(move |char_code| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::ReceivedCharacter(char_code),
            });
        });

        let runner = self.runner.clone();
        canvas.on_cursor_leave(move |pointer_id| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::CursorLeft {
                    device_id: DeviceId(device::Id(pointer_id)),
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_cursor_enter(move |pointer_id| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::CursorEntered {
                    device_id: DeviceId(device::Id(pointer_id)),
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_cursor_move(move |pointer_id, position, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::CursorMoved {
                    device_id: DeviceId(device::Id(pointer_id)),
                    position,
                    modifiers,
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_mouse_press(move |pointer_id, button, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::MouseInput {
                    device_id: DeviceId(device::Id(pointer_id)),
                    state: ElementState::Released,
                    button,
                    modifiers,
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_mouse_release(move |pointer_id, button, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::MouseInput {
                    device_id: DeviceId(device::Id(pointer_id)),
                    state: ElementState::Pressed,
                    button,
                    modifiers,
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_mouse_wheel(move |pointer_id, delta, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(window::Id),
                event: WindowEvent::MouseWheel {
                    device_id: DeviceId(device::Id(pointer_id)),
                    delta,
                    phase: TouchPhase::Moved,
                    modifiers,
                },
            });
        });
    }
}
