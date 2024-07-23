use std::cell::Cell;
use std::clone::Clone;
use std::iter;
use std::rc::Rc;

use web_sys::Element;

use super::super::monitor::{MonitorHandle, MonitorPermissionFuture};
use super::super::{lock, KeyEventExtra};
use super::device::DeviceId;
use super::runner::{EventWrapper, WeakShared};
use super::window::WindowId;
use super::{backend, runner, EventLoopProxy};
use crate::error::NotSupportedError;
use crate::event::{
    DeviceId as RootDeviceId, ElementState, Event, KeyEvent, Touch, TouchPhase, WindowEvent,
};
use crate::event_loop::{ControlFlow, DeviceEvents};
use crate::keyboard::ModifiersState;
use crate::platform::web::{CustomCursorFuture, PollStrategy, WaitUntilStrategy};
use crate::platform_impl::platform::cursor::CustomCursor;
use crate::platform_impl::platform::r#async::Waker;
use crate::window::{
    CustomCursor as RootCustomCursor, CustomCursorSource, Theme, WindowId as RootWindowId,
};

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

#[derive(Clone)]
pub struct ActiveEventLoop {
    pub(crate) runner: runner::Shared,
    modifiers: ModifiersShared,
}

impl ActiveEventLoop {
    pub fn new() -> Self {
        Self { runner: runner::Shared::new(), modifiers: ModifiersShared::default() }
    }

    pub(crate) fn run(
        &self,
        event_handler: Box<runner::EventHandler>,
        event_loop_recreation: bool,
    ) {
        self.runner.event_loop_recreation(event_loop_recreation);
        self.runner.start(event_handler);
    }

    pub fn generate_id(&self) -> WindowId {
        WindowId(self.runner.generate_id())
    }

    pub fn create_proxy(&self) -> EventLoopProxy {
        EventLoopProxy::new(self.waker())
    }

    pub fn create_custom_cursor(&self, source: CustomCursorSource) -> RootCustomCursor {
        RootCustomCursor { inner: CustomCursor::new(self, source.inner) }
    }

    pub fn create_custom_cursor_async(&self, source: CustomCursorSource) -> CustomCursorFuture {
        CustomCursorFuture(CustomCursor::new_async(self, source.inner))
    }

    pub fn register(&self, canvas: &Rc<backend::Canvas>, id: WindowId) {
        let canvas_clone = canvas.clone();
        #[cfg(any(feature = "rwh_04", feature = "rwh_05"))]
        canvas.set_attribute("data-raw-handle", &id.0.to_string());

        canvas.on_touch_start();

        let runner = self.runner.clone();
        let has_focus = canvas.has_focus.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_blur(move || {
            has_focus.set(false);

            let clear_modifiers = (!modifiers.get().is_empty()).then(|| {
                modifiers.set(ModifiersState::empty());
                Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::ModifiersChanged(ModifiersState::empty().into()),
                }
            });

            runner.send_events(clear_modifiers.into_iter().chain(iter::once(Event::WindowEvent {
                window_id: RootWindowId(id),
                event: WindowEvent::Focused(false),
            })));
        });

        let runner = self.runner.clone();
        let has_focus = canvas.has_focus.clone();
        canvas.on_focus(move || {
            if !has_focus.replace(true) {
                runner.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::Focused(true),
                });
            }
        });

        // It is possible that at this point the canvas has
        // been focused before the callback can be called.
        let focused = canvas
            .document()
            .active_element()
            .filter(|element| {
                let canvas: &Element = canvas.raw();
                element == canvas
            })
            .is_some();

        if focused {
            canvas.has_focus.set(true);
            self.runner.send_event(Event::WindowEvent {
                window_id: RootWindowId(id),
                event: WindowEvent::Focused(true),
            })
        }

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

                let device_id = RootDeviceId(DeviceId::dummy());

                runner.send_events(
                    iter::once(Event::WindowEvent {
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
                    })
                    .chain(modifiers_changed),
                );
            },
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

                let device_id = RootDeviceId(DeviceId::dummy());

                runner.send_events(
                    iter::once(Event::WindowEvent {
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
                    })
                    .chain(modifiers_changed),
                )
            },
        );

        let has_focus = canvas.has_focus.clone();
        canvas.on_cursor_leave({
            let runner = self.runner.clone();
            let has_focus = has_focus.clone();
            let modifiers = self.modifiers.clone();

            move |active_modifiers, pointer_id| {
                let focus = (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
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
                let focus = (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
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

                move |active_modifiers, pointer_id, events| {
                    let modifiers =
                        (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
                            modifiers.set(active_modifiers);
                            Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                            }
                        });

                    runner.send_events(modifiers.into_iter().chain(events.flat_map(|position| {
                        let device_id = RootDeviceId(DeviceId(pointer_id));

                        iter::once(Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::CursorMoved { device_id, position },
                        })
                    })));
                }
            },
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers, device_id, events| {
                    let modifiers =
                        (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
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
                    let modifiers =
                        (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
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

                    // A chorded button event may come in without any prior CursorMoved events,
                    // therefore we should send a CursorMoved event to make sure that the
                    // user code has the correct cursor position.
                    runner.send_events(modifiers.into_iter().chain([
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::CursorMoved { device_id, position },
                        },
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::MouseInput { device_id, state, button },
                        },
                    ]));
                }
            },
        );

        canvas.on_mouse_press(
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

                    // A mouse down event may come in without any prior CursorMoved events,
                    // therefore we should send a CursorMoved event to make sure that the
                    // user code has the correct cursor position.
                    runner.send_events(modifiers.into_iter().chain([
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::CursorMoved { device_id, position },
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
        );

        canvas.on_mouse_release(
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers, pointer_id, position, button| {
                    let modifiers =
                        (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
                            modifiers.set(active_modifiers);
                            Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                            }
                        });

                    let device_id: RootDeviceId = RootDeviceId(DeviceId(pointer_id));

                    // A mouse up event may come in without any prior CursorMoved events,
                    // therefore we should send a CursorMoved event to make sure that the
                    // user code has the correct cursor position.
                    runner.send_events(modifiers.into_iter().chain([
                        Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::CursorMoved { device_id, position },
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
                    let modifiers =
                        (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
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
        canvas.on_mouse_wheel(move |pointer_id, delta, active_modifiers| {
            let modifiers_changed =
                (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
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
            let theme = if is_dark_mode { Theme::Dark } else { Theme::Light };
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
                let canvas = canvas_clone.clone();

                move |new_size| {
                    canvas.set_current_size(new_size);
                    if canvas.old_size() != new_size {
                        canvas.set_old_size(new_size);
                        runner.send_event(Event::WindowEvent {
                            window_id: RootWindowId(id),
                            event: WindowEvent::Resized(new_size),
                        });
                        canvas.request_animation_frame();
                    }
                }
            },
        );

        let runner = self.runner.clone();
        canvas.on_intersection(move |is_intersecting| {
            // only fire if visible while skipping the first event if it's intersecting
            if backend::is_visible(runner.document())
                && !(is_intersecting && canvas_clone.is_intersecting.get().is_none())
            {
                runner.send_event(Event::WindowEvent {
                    window_id: RootWindowId(id),
                    event: WindowEvent::Occluded(!is_intersecting),
                });
            }

            canvas_clone.is_intersecting.set(Some(is_intersecting));
        });

        let runner = self.runner.clone();
        canvas.on_animation_frame(move || runner.request_redraw(RootWindowId(id)));

        canvas.on_context_menu();
    }

    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        self.runner.monitor().available_monitors()
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        self.runner.monitor().primary_monitor()
    }

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::RawDisplayHandle::Web(rwh_05::WebDisplayHandle::empty())
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Web(rwh_06::WebDisplayHandle::new()))
    }

    pub fn listen_device_events(&self, allowed: DeviceEvents) {
        self.runner.listen_device_events(allowed)
    }

    pub(crate) fn set_control_flow(&self, control_flow: ControlFlow) {
        self.runner.set_control_flow(control_flow)
    }

    pub(crate) fn control_flow(&self) -> ControlFlow {
        self.runner.control_flow()
    }

    pub(crate) fn exit(&self) {
        self.runner.exit()
    }

    pub(crate) fn exiting(&self) -> bool {
        self.runner.exiting()
    }

    pub(crate) fn set_poll_strategy(&self, strategy: PollStrategy) {
        self.runner.set_poll_strategy(strategy)
    }

    pub(crate) fn poll_strategy(&self) -> PollStrategy {
        self.runner.poll_strategy()
    }

    pub(crate) fn set_wait_until_strategy(&self, strategy: WaitUntilStrategy) {
        self.runner.set_wait_until_strategy(strategy)
    }

    pub(crate) fn wait_until_strategy(&self) -> WaitUntilStrategy {
        self.runner.wait_until_strategy()
    }

    pub(crate) fn is_cursor_lock_raw(&self) -> bool {
        lock::is_cursor_lock_raw(self.runner.navigator(), self.runner.document())
    }

    pub(crate) fn has_multiple_screens(&self) -> Result<bool, NotSupportedError> {
        self.runner.monitor().is_extended().ok_or(NotSupportedError::new())
    }

    pub(crate) fn request_detailed_monitor_permission(&self) -> MonitorPermissionFuture {
        self.runner.monitor().request_detailed_monitor_permission(self.runner.weak())
    }

    pub(crate) fn has_detailed_monitor_permission(&self) -> bool {
        self.runner.monitor().has_detailed_monitor_permission()
    }

    pub(crate) fn waker(&self) -> Waker<WeakShared> {
        self.runner.waker()
    }

    pub(crate) fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle
    }
}

#[derive(Clone)]
pub(crate) struct OwnedDisplayHandle;

impl OwnedDisplayHandle {
    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::WebDisplayHandle::empty().into()
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::WebDisplayHandle::new().into())
    }
}
