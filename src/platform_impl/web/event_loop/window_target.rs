use super::{super::monitor, backend, device, proxy::Proxy, runner, window};
use crate::dpi::{PhysicalSize, Size};
use crate::event::{DeviceId, ElementState, Event, KeyboardInput, TouchPhase, WindowEvent};
use crate::event_loop::ControlFlow;
use crate::{
    platform_impl::CanvasResizeChangedFlag,
    window::{Theme, WindowId},
};
use std::clone::Clone;
use std::collections::{vec_deque::IntoIter as VecDequeIter, VecDeque};

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
        let is_auto_parent_size = canvas.auto_parent_size;

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

        let runner = self.runner.clone();
        let raw = canvas.raw().clone();
        let mut old_dpr = backend::scale_factor();
        canvas.on_device_pixel_ratio_change(move || {
            let new_dpr = backend::scale_factor();
            let current_size = PhysicalSize {
                width: raw.width() as u32,
                height: raw.height() as u32,
            };
            let logical_size = current_size.to_logical::<f64>(old_dpr);
            let new_size = logical_size.to_physical(new_dpr);

            // TODO: Remove debugging output
            #[cfg(feature = "web-sys")]
            {
                web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
                    "devicePixelRatio changed from {:?} to {:?}",
                    old_dpr, new_dpr,
                )));
                web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
                    "old size {:?} -> new size {:?}",
                    current_size, new_size,
                )));
            }

            backend::set_canvas_size(&raw, Size::Physical(new_size));

            // TODO: How to handle the `new_inner_size`?
            let size = Box::leak(Box::new(new_size.clone()));
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::ScaleFactorChanged {
                    scale_factor: new_dpr,
                    new_inner_size: size,
                },
            });
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::Resized(new_size),
            });
            runner.request_redraw(WindowId(id));
            old_dpr = new_dpr;
        });

        let runner = self.runner.clone();
        canvas.on_size_or_scale_change(move |args| {
            if args.changed_flag == CanvasResizeChangedFlag::SizeAndDevicePixelRatioChanged {
                // TODO: Remove debugging output
                #[cfg(feature = "web-sys")]
                web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
                    "devicePixelRatio changed to {:?}",
                    args.device_pixel_ratio
                )));
                // TODO: How to handle the `new_inner_size`?
                let size = Box::leak(Box::new(args.size));
                runner.send_event(Event::WindowEvent {
                    window_id: WindowId(id),
                    event: WindowEvent::ScaleFactorChanged {
                        scale_factor: args.device_pixel_ratio,
                        new_inner_size: size,
                    },
                });
            }
            #[cfg(feature = "web-sys")]
            web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(
                &format!("canvas resized",),
            ));
            runner.send_event(Event::WindowEvent {
                window_id: WindowId(id),
                event: WindowEvent::Resized(args.size),
            });
            runner.request_redraw(WindowId(id));
        });
    }

    pub fn available_monitors(&self) -> VecDequeIter<monitor::Handle> {
        VecDeque::new().into_iter()
    }

    pub fn primary_monitor(&self) -> monitor::Handle {
        monitor::Handle
    }
}
