use std::cell::RefCell;
use std::clone::Clone;
use std::collections::{vec_deque::IntoIter as VecDequeIter, VecDeque};
use std::rc::Rc;

use raw_window_handle::{RawDisplayHandle, WebDisplayHandle};

use super::{
    super::monitor::MonitorHandle, backend, device::DeviceId, proxy::EventLoopProxy, runner,
    window::WindowId,
};
use crate::dpi::{PhysicalSize, Size};
use crate::event::{
    DeviceEvent, DeviceId as RootDeviceId, ElementState, Event, KeyboardInput, Touch, TouchPhase,
    WindowEvent,
};
use crate::window::{Theme, WindowId as RootWindowId};

pub struct EventLoopWindowTarget<T: 'static> {
    pub(crate) runner: runner::Shared<T>,
}

impl<T> Clone for EventLoopWindowTarget<T> {
    fn clone(&self) -> Self {
        Self {
            runner: self.runner.clone(),
        }
    }
}

impl<T> EventLoopWindowTarget<T> {
    pub fn new() -> Self {
        Self {
            runner: runner::Shared::new(),
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
        has_focus: Rc<RefCell<bool>>,
    ) {
        self.runner.add_canvas(RootWindowId(id), canvas);
        let mut canvas = canvas.borrow_mut();
        canvas.set_attribute("data-raw-handle", &id.0.to_string());

        canvas.on_touch_start(prevent_default);
        canvas.on_touch_end(prevent_default);

        let runner = self.runner.clone();
        let has_focus_clone = has_focus.clone();
        canvas.on_blur(move || {
            *has_focus_clone.borrow_mut() = false;
            runner.send_event(Event::WindowEvent {
                window_id: RootWindowId(id),
                event: WindowEvent::Focused(false),
            });
        });

        let runner = self.runner.clone();
        let has_focus_clone = has_focus.clone();
        canvas.on_focus(move || {
            *has_focus_clone.borrow_mut() = true;
            runner.send_event(Event::WindowEvent {
                window_id: RootWindowId(id),
                event: WindowEvent::Focused(true),
            });
        });

        let runner = self.runner.clone();
        canvas.on_keyboard_press(
            move |scancode, virtual_keycode, modifiers| {
                #[allow(deprecated)]
                runner.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::KeyboardInput {
                        device_id: RootDeviceId(unsafe { DeviceId::dummy() }),
                        input: KeyboardInput {
                            scancode,
                            state: ElementState::Pressed,
                            virtual_keycode,
                            modifiers,
                        },
                        is_synthetic: false,
                    },
                });
            },
            prevent_default,
        );

        let runner = self.runner.clone();
        canvas.on_keyboard_release(
            move |scancode, virtual_keycode, modifiers| {
                #[allow(deprecated)]
                runner.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::KeyboardInput {
                        device_id: RootDeviceId(unsafe { DeviceId::dummy() }),
                        input: KeyboardInput {
                            scancode,
                            state: ElementState::Released,
                            virtual_keycode,
                            modifiers,
                        },
                        is_synthetic: false,
                    },
                });
            },
            prevent_default,
        );

        let runner = self.runner.clone();
        canvas.on_received_character(
            move |char_code| {
                runner.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::ReceivedCharacter(char_code),
                });
            },
            prevent_default,
        );

        let runner = self.runner.clone();
        canvas.on_cursor_leave(move |pointer_id| {
            runner.send_event(Event::WindowEvent {
                window_id: RootWindowId(id),
                event: WindowEvent::CursorLeft {
                    device_id: RootDeviceId(DeviceId(pointer_id)),
                },
            });
        });

        let runner = self.runner.clone();
        canvas.on_cursor_enter(move |pointer_id| {
            runner.send_event(Event::WindowEvent {
                window_id: RootWindowId(id),
                event: WindowEvent::CursorEntered {
                    device_id: RootDeviceId(DeviceId(pointer_id)),
                },
            });
        });

        let runner = self.runner.clone();
        let runner_touch = self.runner.clone();
        canvas.on_cursor_move(
            move |pointer_id, position, delta, modifiers| {
                runner.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::CursorMoved {
                        device_id: RootDeviceId(DeviceId(pointer_id)),
                        position,
                        modifiers,
                    },
                });
                runner.send_event(Event::DeviceEvent {
                    device_id: RootDeviceId(DeviceId(pointer_id)),
                    event: DeviceEvent::MouseMotion {
                        delta: (delta.x, delta.y),
                    },
                });
            },
            move |device_id, location, force| {
                runner_touch.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::Touch(Touch {
                        id: device_id as u64,
                        device_id: RootDeviceId(DeviceId(device_id)),
                        phase: TouchPhase::Moved,
                        force: Some(force),
                        location,
                    }),
                });
            },
            prevent_default,
        );

        let runner = self.runner.clone();
        let runner_touch = self.runner.clone();
        canvas.on_mouse_press(
            move |pointer_id, position, button, modifiers| {
                *has_focus.borrow_mut() = true;

                // A mouse down event may come in without any prior CursorMoved events,
                // therefore we should send a CursorMoved event to make sure that the
                // user code has the correct cursor position.
                runner.send_events(
                    std::iter::once(Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::Focused(true),
                    })
                    .chain(std::iter::once(Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::CursorMoved {
                            device_id: RootDeviceId(DeviceId(pointer_id)),
                            position,
                            modifiers,
                        },
                    }))
                    .chain(std::iter::once(Event::WindowEvent {
                        window_id: RootWindowId(id),
                        event: WindowEvent::MouseInput {
                            device_id: RootDeviceId(DeviceId(pointer_id)),
                            state: ElementState::Pressed,
                            button,
                            modifiers,
                        },
                    })),
                );
            },
            move |device_id, location, force| {
                runner_touch.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::Touch(Touch {
                        id: device_id as u64,
                        device_id: RootDeviceId(DeviceId(device_id)),
                        phase: TouchPhase::Started,
                        force: Some(force),
                        location,
                    }),
                });
            },
        );

        let runner = self.runner.clone();
        let runner_touch = self.runner.clone();
        canvas.on_mouse_release(
            move |pointer_id, button, modifiers| {
                runner.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::MouseInput {
                        device_id: RootDeviceId(DeviceId(pointer_id)),
                        state: ElementState::Released,
                        button,
                        modifiers,
                    },
                });
            },
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
            },
        );

        let runner = self.runner.clone();
        canvas.on_mouse_wheel(
            move |pointer_id, delta, modifiers| {
                runner.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::MouseWheel {
                        device_id: RootDeviceId(DeviceId(pointer_id)),
                        delta,
                        phase: TouchPhase::Moved,
                        modifiers,
                    },
                });
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
