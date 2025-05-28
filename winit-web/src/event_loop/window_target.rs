use std::cell::Cell;
use std::clone::Clone;
use std::iter;
use std::rc::Rc;
use std::sync::Arc;

use web_sys::Element;
use winit_core::application::ApplicationHandler;
use winit_core::cursor::{CustomCursor as CoreCustomCursor, CustomCursorSource};
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event::{ElementState, KeyEvent, TouchPhase, WindowEvent};
use winit_core::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents,
    EventLoopProxy as RootEventLoopProxy, OwnedDisplayHandle as CoreOwnedDisplayHandle,
};
use winit_core::keyboard::ModifiersState;
use winit_core::monitor::MonitorHandle as CoremMonitorHandle;
use winit_core::window::{Theme, SurfaceId};

use super::super::lock;
use super::super::monitor::MonitorPermissionFuture;
use super::runner::Event;
use super::{backend, runner};
use crate::cursor::CustomCursor;
use crate::event_loop::proxy::EventLoopProxy;
use crate::window::Window;
use crate::{CustomCursorFuture, PollStrategy, WaitUntilStrategy};

#[derive(Default, Debug)]
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

#[derive(Clone, Debug)]
pub struct ActiveEventLoop {
    pub(crate) runner: runner::Shared,
    modifiers: ModifiersShared,
}

impl ActiveEventLoop {
    pub fn new() -> Self {
        Self { runner: runner::Shared::new(), modifiers: ModifiersShared::default() }
    }

    pub(crate) fn run(&self, app: Box<dyn ApplicationHandler>, event_loop_recreation: bool) {
        self.runner.event_loop_recreation(event_loop_recreation);
        self.runner.start(app, self.clone());
    }

    pub fn generate_id(&self) -> SurfaceId {
        SurfaceId::from_raw(self.runner.generate_id())
    }

    pub fn create_custom_cursor_async(&self, source: CustomCursorSource) -> CustomCursorFuture {
        CustomCursorFuture(CustomCursor::new_async(self, source))
    }

    pub fn register(&self, canvas: &Rc<backend::Canvas>, window_id: SurfaceId) {
        let canvas_clone = canvas.clone();

        canvas.on_touch_start();

        let runner = self.runner.clone();
        let has_focus = canvas.has_focus.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_blur(move || {
            has_focus.set(false);

            let clear_modifiers = (!modifiers.get().is_empty()).then(|| {
                modifiers.set(ModifiersState::empty());
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::ModifiersChanged(ModifiersState::empty().into()),
                }
            });

            runner.send_events(clear_modifiers.into_iter().chain(iter::once(Event::WindowEvent {
                window_id,
                event: WindowEvent::Focused(false),
            })));
        });

        let runner = self.runner.clone();
        let has_focus = canvas.has_focus.clone();
        canvas.on_focus(move || {
            if !has_focus.replace(true) {
                runner.send_event(Event::WindowEvent {
                    window_id,
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
            self.runner
                .send_event(Event::WindowEvent { window_id, event: WindowEvent::Focused(true) })
        }

        let runner = self.runner.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_keyboard_press(
            move |physical_key, logical_key, text, location, repeat, active_modifiers| {
                let modifiers_changed = (modifiers.get() != active_modifiers).then(|| {
                    modifiers.set(active_modifiers);
                    Event::WindowEvent {
                        window_id,
                        event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                    }
                });

                runner.send_events(
                    iter::once(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::KeyboardInput {
                            device_id: None,
                            event: KeyEvent {
                                physical_key,
                                logical_key: logical_key.clone(),
                                text: text.clone(),
                                location,
                                state: ElementState::Pressed,
                                repeat,
                                text_with_all_modifiers: text,
                                key_without_modifiers: logical_key,
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
                        window_id,
                        event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                    }
                });

                runner.send_events(
                    iter::once(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::KeyboardInput {
                            device_id: None,
                            event: KeyEvent {
                                physical_key,
                                logical_key: logical_key.clone(),
                                text: text.clone(),
                                location,
                                state: ElementState::Released,
                                repeat,
                                text_with_all_modifiers: text,
                                key_without_modifiers: logical_key,
                            },
                            is_synthetic: false,
                        },
                    })
                    .chain(modifiers_changed),
                )
            },
        );

        let has_focus = canvas.has_focus.clone();
        canvas.on_pointer_leave({
            let runner = self.runner.clone();
            let has_focus = has_focus.clone();
            let modifiers = self.modifiers.clone();

            move |active_modifiers, device_id, primary, position, kind| {
                let focus = (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
                    modifiers.set(active_modifiers);
                    Event::WindowEvent {
                        window_id,
                        event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                    }
                });

                runner.send_events(focus.into_iter().chain(iter::once(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::PointerLeft {
                        device_id,
                        primary,
                        position: Some(position),
                        kind,
                    },
                })))
            }
        });

        canvas.on_pointer_enter({
            let runner = self.runner.clone();
            let has_focus = has_focus.clone();
            let modifiers = self.modifiers.clone();

            move |active_modifiers, device_id, primary, position, kind| {
                let focus = (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
                    modifiers.set(active_modifiers);
                    Event::WindowEvent {
                        window_id,
                        event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                    }
                });

                runner.send_events(focus.into_iter().chain(iter::once(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::PointerEntered { device_id, primary, position, kind },
                })))
            }
        });

        canvas.on_pointer_move(
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |device_id, events| {
                    runner.send_events(events.flat_map(
                        |(active_modifiers, primary, position, source)| {
                            let modifiers = (has_focus.get()
                                && modifiers.get() != active_modifiers)
                                .then(|| {
                                    modifiers.set(active_modifiers);
                                    Event::WindowEvent {
                                        window_id,
                                        event: WindowEvent::ModifiersChanged(
                                            active_modifiers.into(),
                                        ),
                                    }
                                });

                            modifiers.into_iter().chain(iter::once(Event::WindowEvent {
                                window_id,
                                event: WindowEvent::PointerMoved {
                                    device_id,
                                    primary,
                                    position,
                                    source,
                                },
                            }))
                        },
                    ));
                }
            },
            {
                let runner = self.runner.clone();
                let has_focus = has_focus.clone();
                let modifiers = self.modifiers.clone();

                move |active_modifiers, device_id, primary, position, state, button| {
                    let modifiers =
                        (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
                            modifiers.set(active_modifiers);
                            Event::WindowEvent {
                                window_id,
                                event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                            }
                        });

                    runner.send_events(modifiers.into_iter().chain([Event::WindowEvent {
                        window_id,
                        event: WindowEvent::PointerButton {
                            device_id,
                            primary,
                            state,
                            position,
                            button,
                        },
                    }]));
                }
            },
        );

        canvas.on_pointer_press({
            let runner = self.runner.clone();
            let modifiers = self.modifiers.clone();

            move |active_modifiers, device_id, primary, position, button| {
                let modifiers = (modifiers.get() != active_modifiers).then(|| {
                    modifiers.set(active_modifiers);
                    Event::WindowEvent {
                        window_id,
                        event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                    }
                });

                runner.send_events(modifiers.into_iter().chain(iter::once(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::PointerButton {
                        device_id,
                        primary,
                        state: ElementState::Pressed,
                        position,
                        button,
                    },
                })));
            }
        });

        canvas.on_pointer_release({
            let runner = self.runner.clone();
            let has_focus = has_focus.clone();
            let modifiers = self.modifiers.clone();

            move |active_modifiers, device_id, primary, position, button| {
                let modifiers =
                    (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
                        modifiers.set(active_modifiers);
                        Event::WindowEvent {
                            window_id,
                            event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                        }
                    });

                runner.send_events(modifiers.into_iter().chain(iter::once(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::PointerButton {
                        device_id,
                        primary,
                        state: ElementState::Released,
                        position,
                        button,
                    },
                })));
            }
        });

        let runner = self.runner.clone();
        let modifiers = self.modifiers.clone();
        canvas.on_mouse_wheel(move |delta, active_modifiers| {
            let modifiers_changed =
                (has_focus.get() && modifiers.get() != active_modifiers).then(|| {
                    modifiers.set(active_modifiers);
                    Event::WindowEvent {
                        window_id,
                        event: WindowEvent::ModifiersChanged(active_modifiers.into()),
                    }
                });

            runner.send_events(modifiers_changed.into_iter().chain(iter::once(
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::MouseWheel {
                        device_id: None,
                        delta,
                        phase: TouchPhase::Moved,
                    },
                },
            )));
        });

        let runner = self.runner.clone();
        canvas.on_dark_mode(move |is_dark_mode| {
            let theme = if is_dark_mode { Theme::Dark } else { Theme::Light };
            runner.send_event(Event::WindowEvent {
                window_id,
                event: WindowEvent::ThemeChanged(theme),
            });
        });

        canvas.on_resize_scale(
            {
                let runner = self.runner.clone();
                let canvas = canvas_clone.clone();

                move |size, scale| {
                    runner.send_event(Event::ScaleChange {
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
                            window_id,
                            event: WindowEvent::SurfaceResized(new_size),
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
                    window_id,
                    event: WindowEvent::Occluded(!is_intersecting),
                });
            }

            canvas_clone.is_intersecting.set(Some(is_intersecting));
        });

        let runner = self.runner.clone();
        canvas.on_animation_frame(move || runner.request_redraw(window_id));

        canvas.on_context_menu();
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
        self.runner
            .monitor()
            .is_extended()
            .ok_or(NotSupportedError::new("has_multiple_screens is not supported"))
    }

    pub(crate) fn request_detailed_monitor_permission(&self) -> MonitorPermissionFuture {
        self.runner.monitor().request_detailed_monitor_permission()
    }

    pub(crate) fn has_detailed_monitor_permission(&self) -> bool {
        self.runner.monitor().has_detailed_monitor_permission()
    }

    pub(crate) fn event_loop_proxy(&self) -> Arc<EventLoopProxy> {
        self.runner.event_loop_proxy().clone()
    }
}

impl RootActiveEventLoop for ActiveEventLoop {
    fn create_proxy(&self) -> RootEventLoopProxy {
        let event_loop_proxy = self.event_loop_proxy();
        RootEventLoopProxy::new(event_loop_proxy)
    }

    fn create_window(
        &self,
        window_attributes: winit_core::window::WindowAttributes,
    ) -> Result<Box<dyn winit_core::window::Window>, RequestError> {
        let window = Window::new(self, window_attributes)?;
        Ok(Box::new(window))
    }

    fn create_custom_cursor(
        &self,
        source: CustomCursorSource,
    ) -> Result<CoreCustomCursor, RequestError> {
        Ok(CoreCustomCursor(Arc::new(CustomCursor::new(self, source))))
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoremMonitorHandle>> {
        Box::new(
            self.runner
                .monitor()
                .available_monitors()
                .into_iter()
                .map(|monitor| CoremMonitorHandle(Arc::new(monitor))),
        )
    }

    fn primary_monitor(&self) -> Option<CoremMonitorHandle> {
        self.runner.monitor().primary_monitor().map(|monitor| CoremMonitorHandle(Arc::new(monitor)))
    }

    fn listen_device_events(&self, allowed: DeviceEvents) {
        self.runner.listen_device_events(allowed)
    }

    fn system_theme(&self) -> Option<Theme> {
        backend::is_dark_mode(self.runner.window()).map(|is_dark_mode| {
            if is_dark_mode {
                Theme::Dark
            } else {
                Theme::Light
            }
        })
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.runner.set_control_flow(control_flow)
    }

    fn control_flow(&self) -> ControlFlow {
        self.runner.control_flow()
    }

    fn exit(&self) {
        self.runner.exit()
    }

    fn exiting(&self) -> bool {
        self.runner.exiting()
    }

    fn owned_display_handle(&self) -> CoreOwnedDisplayHandle {
        CoreOwnedDisplayHandle::new(Arc::new(OwnedDisplayHandle))
    }

    fn rwh_06_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }
}

impl rwh_06::HasDisplayHandle for ActiveEventLoop {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::RawDisplayHandle::Web(rwh_06::WebDisplayHandle::new());
        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw)) }
    }
}

#[derive(Clone)]
pub(crate) struct OwnedDisplayHandle;

impl rwh_06::HasDisplayHandle for OwnedDisplayHandle {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::RawDisplayHandle::Web(rwh_06::WebDisplayHandle::new());
        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw)) }
    }
}
