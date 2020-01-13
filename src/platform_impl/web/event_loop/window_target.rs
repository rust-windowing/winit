use super::{backend, device, proxy::Proxy, runner, window};
use crate::dpi::{PhysicalSize, Size};
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

    pub fn run(&self, event_handler: Box<dyn FnMut(Event<'static, T>, &mut ControlFlow)>) {
        self.runner.set_listener(event_handler);
    }

    pub fn generate_id(&self) -> window::Id {
        window::Id(self.runner.generate_id())
    }

    pub fn register(&self, canvas: &mut backend::Canvas, id: window::Id) {
        let runner = self.runner.clone();
        canvas.set_attribute("data-raw-handle", &id.0.to_string());

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
        canvas.on_keyboard_press(move |scancode, virtual_keycode, modifiers| {
            #[allow(deprecated)]
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::KeyboardInput {
                    device_id: DeviceId(unsafe { device::Id::dummy() }),
                    input: KeyboardInput {
                        scancode,
                        state: ElementState::Pressed,
                        virtual_keycode,
                        modifiers,
                    },
                    is_synthetic: false,
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_keyboard_release(move |scancode, virtual_keycode, modifiers| {
            #[allow(deprecated)]
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::KeyboardInput {
                    device_id: DeviceId(unsafe { device::Id::dummy() }),
                    input: KeyboardInput {
                        scancode,
                        state: ElementState::Released,
                        virtual_keycode,
                        modifiers,
                    },
                    is_synthetic: false,
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_received_character(move |char_code| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::ReceivedCharacter(char_code),
            });
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
        canvas.on_cursor_move(move |pointer_id, position, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
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
                window_id: WindowId(id),
                event: WindowEvent::MouseInput {
                    device_id: DeviceId(device::Id(pointer_id)),
                    state: ElementState::Pressed,
                    button,
                    modifiers,
                },
            });
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
    }
}
