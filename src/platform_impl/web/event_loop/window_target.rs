use super::{backend, proxy::Proxy, runner, window};
use crate::dpi::LogicalSize;
use crate::event::{device, ElementState, Event, KeyboardInput, WindowEvent};
use crate::event_loop::ControlFlow;
use crate::platform_impl::platform::device::{GamepadHandle, SharedGamepad, KeyboardId, MouseId};
use crate::window::WindowId;

pub struct WindowTarget<T: 'static> {
    pub(crate) runner: runner::Shared<T>,
    pub(crate) shared_window: backend::SharedWindow,
}

impl<T> Clone for WindowTarget<T> {
    fn clone(&self) -> Self {
        WindowTarget {
            runner: self.runner.clone(),
            shared_window: self.shared_window.clone(),
        }
    }
}

impl<T> WindowTarget<T> {
    pub fn new() -> Self {
        WindowTarget {
            runner: runner::Shared::new(),
            shared_window: backend::SharedWindow::new(),
        }
    }

    pub fn proxy(&self) -> Proxy<T> {
        Proxy::new(self.runner.clone())
    }

    pub fn run(&self, event_handler: Box<dyn FnMut(Event<T>, &mut ControlFlow)>) {
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
            runner.send_event(Event::KeyboardEvent(
                device::KeyboardId(unsafe { KeyboardId::dummy() }),
                device::KeyboardEvent::Input(KeyboardInput {
                    scancode,
                    state: ElementState::Pressed,
                    virtual_keycode,
                    modifiers,
                }),
            ));
        });

        let runner = self.runner.clone();
        canvas.on_keyboard_release(move |scancode, virtual_keycode, modifiers| {
            runner.send_event(Event::KeyboardEvent(
                device::KeyboardId(unsafe { KeyboardId::dummy() }),
                device::KeyboardEvent::Input(KeyboardInput {
                    scancode,
                    state: ElementState::Released,
                    virtual_keycode,
                    modifiers,
                }),
            ));
        });

        let runner = self.runner.clone();
        canvas.on_received_character(move |char_code| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::ReceivedCharacter(char_code),
            });
        });

        let runner = self.runner.clone();
        canvas.on_cursor_leave(move || {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::CursorLeft,
            });
        });

        let runner = self.runner.clone();
        canvas.on_cursor_enter(move || {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::CursorEntered,
            });
        });

        let runner = self.runner.clone();
        canvas.on_cursor_move(move |position, modifiers| {
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::CursorMoved {
                    position,
                    modifiers,
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_mouse_press(move |pointer_id, button| {
            runner.send_event(Event::MouseEvent(
                device::MouseId(MouseId(pointer_id)),
                device::MouseEvent::Button {
                    state: ElementState::Pressed,
                    button,
                },
            ));
        });

        let runner = self.runner.clone();
        canvas.on_mouse_release(move |pointer_id, button| {
            runner.send_event(Event::MouseEvent(
                device::MouseId(MouseId(pointer_id)),
                device::MouseEvent::Button {
                    state: ElementState::Released,
                    button,
                },
            ));
        });

        let runner = self.runner.clone();
        canvas.on_mouse_wheel(move |pointer_id, delta| {
            runner.send_event(Event::MouseEvent(
                device::MouseId(MouseId(pointer_id)),
                device::MouseEvent::Wheel(delta.0, delta.1),
            ));
        });

        let runner = self.runner.clone();
        let raw = canvas.raw().clone();
        let mut intended_size = LogicalSize {
            width: raw.width() as f64,
            height: raw.height() as f64,
        };
        canvas.on_fullscreen_change(move || {
            // If the canvas is marked as fullscreen, it is moving *into* fullscreen
            // If it is not, it is moving *out of* fullscreen
            let new_size = if backend::is_fullscreen(&raw) {
                intended_size = LogicalSize {
                    width: raw.width() as f64,
                    height: raw.height() as f64,
                };

                backend::window_size()
            } else {
                intended_size
            };
            raw.set_width(new_size.width as u32);
            raw.set_height(new_size.height as u32);
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::Resized(new_size),
            });
            runner.request_redraw(WindowId(id));
        });

        let shared_window = self.shared_window.clone();
        let mut window = shared_window.0.borrow_mut();

        let runner = self.runner.clone();
        window.on_gamepad_connected(move |gamepad: backend::SharedGamepad| {
            runner.send_event(Event::GamepadEvent(
                device::GamepadHandle(GamepadHandle {
                    id: gamepad.index() as i32,
                    gamepad: SharedGamepad::Raw(gamepad),
                }),
                device::GamepadEvent::Added,
            ));
        });

        let runner = self.runner.clone();
        window.on_gamepad_disconnected(move |gamepad: backend::SharedGamepad| {
            runner.send_event(Event::GamepadEvent(
                device::GamepadHandle(GamepadHandle {
                    id: gamepad.index() as i32,
                    gamepad: SharedGamepad::Raw(gamepad),
                }),
                device::GamepadEvent::Removed,
            ));
        });
    }
}
