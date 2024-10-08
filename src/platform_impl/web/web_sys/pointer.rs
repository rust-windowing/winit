use std::cell::Cell;
use std::rc::Rc;

use web_sys::PointerEvent;

use super::super::event::DeviceId;
use super::canvas::Common;
use super::event;
use super::event_handle::EventListenerHandle;
use crate::dpi::PhysicalPosition;
use crate::event::{ButtonSource, ElementState, Force, PointerKind, PointerSource};
use crate::keyboard::ModifiersState;

#[allow(dead_code)]
pub(super) struct PointerHandler {
    on_cursor_leave: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_cursor_enter: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_cursor_move: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_pointer_press: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_pointer_release: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_touch_cancel: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
}

impl PointerHandler {
    pub fn new() -> Self {
        Self {
            on_cursor_leave: None,
            on_cursor_enter: None,
            on_cursor_move: None,
            on_pointer_press: None,
            on_pointer_release: None,
            on_touch_cancel: None,
        }
    }

    pub fn on_pointer_leave<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static + FnMut(ModifiersState, Option<DeviceId>, PhysicalPosition<f64>, PointerKind),
    {
        let window = canvas_common.window.clone();
        self.on_cursor_leave =
            Some(canvas_common.add_event("pointerout", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let device_id = DeviceId::new(pointer_id);
                let position =
                    event::mouse_position(&event).to_physical(super::scale_factor(&window));
                let kind = event::pointer_type(&event, pointer_id);
                handler(modifiers, device_id, position, kind);
            }));
    }

    pub fn on_pointer_enter<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static + FnMut(ModifiersState, Option<DeviceId>, PhysicalPosition<f64>, PointerKind),
    {
        let window = canvas_common.window.clone();
        self.on_cursor_enter =
            Some(canvas_common.add_event("pointerover", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let device_id = DeviceId::new(pointer_id);
                let position =
                    event::mouse_position(&event).to_physical(super::scale_factor(&window));
                let kind = event::pointer_type(&event, pointer_id);
                handler(modifiers, device_id, position, kind);
            }));
    }

    pub fn on_pointer_release<C>(&mut self, canvas_common: &Common, mut handler: C)
    where
        C: 'static + FnMut(ModifiersState, Option<DeviceId>, PhysicalPosition<f64>, ButtonSource),
    {
        let window = canvas_common.window.clone();
        self.on_pointer_release =
            Some(canvas_common.add_event("pointerup", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let kind = event::pointer_type(&event, pointer_id);

                let button = event::mouse_button(&event).expect("no mouse button pressed");

                let source = match kind {
                    PointerKind::Mouse => ButtonSource::Mouse(button),
                    PointerKind::Touch(finger_id) => ButtonSource::Touch {
                        finger_id,
                        force: Some(Force::Normalized(event.pressure().into())),
                    },
                    PointerKind::Unknown => ButtonSource::Unknown(button.to_id()),
                };

                handler(
                    modifiers,
                    DeviceId::new(pointer_id),
                    event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                    source,
                )
            }));
    }

    pub fn on_pointer_press<C>(
        &mut self,
        canvas_common: &Common,
        mut handler: C,
        prevent_default: Rc<Cell<bool>>,
    ) where
        C: 'static + FnMut(ModifiersState, Option<DeviceId>, PhysicalPosition<f64>, ButtonSource),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_pointer_press =
            Some(canvas_common.add_event("pointerdown", move |event: PointerEvent| {
                if prevent_default.get() {
                    // prevent text selection
                    event.prevent_default();
                    // but still focus element
                    let _ = canvas.focus();
                }

                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let kind = event::pointer_type(&event, pointer_id);
                let button = event::mouse_button(&event).expect("no mouse button pressed");

                let source = match kind {
                    PointerKind::Mouse => {
                        // Error is swallowed here since the error would occur every time the
                        // mouse is clicked when the cursor is
                        // grabbed, and there is probably not a
                        // situation where this could fail, that we
                        // care if it fails.
                        let _e = canvas.set_pointer_capture(pointer_id);

                        ButtonSource::Mouse(button)
                    },
                    PointerKind::Touch(finger_id) => ButtonSource::Touch {
                        finger_id,
                        force: Some(Force::Normalized(event.pressure().into())),
                    },
                    PointerKind::Unknown => ButtonSource::Unknown(button.to_id()),
                };

                handler(
                    modifiers,
                    DeviceId::new(pointer_id),
                    event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                    source,
                )
            }));
    }

    pub fn on_pointer_move<C, B>(
        &mut self,
        canvas_common: &Common,
        mut cursor_handler: C,
        mut button_handler: B,
        prevent_default: Rc<Cell<bool>>,
    ) where
        C: 'static
            + FnMut(
                Option<DeviceId>,
                &mut dyn Iterator<Item = (ModifiersState, PhysicalPosition<f64>, PointerSource)>,
            ),
        B: 'static
            + FnMut(
                ModifiersState,
                Option<DeviceId>,
                PhysicalPosition<f64>,
                ElementState,
                ButtonSource,
            ),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_cursor_move =
            Some(canvas_common.add_event("pointermove", move |event: PointerEvent| {
                let pointer_id = event.pointer_id();
                let device_id = DeviceId::new(pointer_id);
                let kind = event::pointer_type(&event, pointer_id);

                // chorded button event
                if let Some(button) = event::mouse_button(&event) {
                    if prevent_default.get() {
                        // prevent text selection
                        event.prevent_default();
                        // but still focus element
                        let _ = canvas.focus();
                    }

                    let state = if event::mouse_buttons(&event).contains(button.into()) {
                        ElementState::Pressed
                    } else {
                        ElementState::Released
                    };

                    let button = match kind {
                        PointerKind::Mouse => ButtonSource::Mouse(button),
                        PointerKind::Touch(finger_id) => {
                            let button_id = button.to_id();

                            if button_id != 1 {
                                tracing::error!("unexpected touch button id: {button_id}");
                            }

                            ButtonSource::Touch {
                                finger_id,
                                force: Some(Force::Normalized(event.pressure().into())),
                            }
                        },
                        PointerKind::Unknown => todo!(),
                    };

                    button_handler(
                        event::mouse_modifiers(&event),
                        device_id,
                        event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                        state,
                        button,
                    );

                    return;
                }

                // pointer move event
                let scale = super::scale_factor(&window);

                cursor_handler(
                    device_id,
                    &mut event::pointer_move_event(event).map(|event| {
                        (
                            event::mouse_modifiers(&event),
                            event::mouse_position(&event).to_physical(scale),
                            match kind {
                                PointerKind::Mouse => PointerSource::Mouse,
                                PointerKind::Touch(finger_id) => PointerSource::Touch {
                                    finger_id,
                                    force: Some(Force::Normalized(event.pressure().into())),
                                },
                                PointerKind::Unknown => PointerSource::Unknown,
                            },
                        )
                    }),
                );
            }));
    }

    pub fn remove_listeners(&mut self) {
        self.on_cursor_leave = None;
        self.on_cursor_enter = None;
        self.on_cursor_move = None;
        self.on_pointer_press = None;
        self.on_pointer_release = None;
        self.on_touch_cancel = None;
    }
}
