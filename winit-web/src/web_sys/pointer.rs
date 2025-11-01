use std::cell::Cell;
use std::rc::Rc;

use dpi::PhysicalPosition;
use web_sys::PointerEvent;
use winit_core::event::{ButtonSource, DeviceId, ElementState, PointerKind, PointerSource};
use winit_core::keyboard::ModifiersState;

use super::canvas::Common;
use super::event::{self, ButtonsState};
use super::event_handle::EventListenerHandle;
use crate::event::mkdid;

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
        F: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, PointerKind),
    {
        let window = canvas_common.window.clone();
        self.on_cursor_leave =
            Some(canvas_common.add_event("pointerout", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let device_id = mkdid(pointer_id);
                let position =
                    event::pointer_position(&event).to_physical(super::scale_factor(&window));
                let kind = event::pointer_kind(&event, pointer_id);
                handler(modifiers, device_id, event.is_primary(), position, kind);
            }));
    }

    pub fn on_pointer_enter<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, PointerKind),
    {
        let window = canvas_common.window.clone();
        self.on_cursor_enter =
            Some(canvas_common.add_event("pointerover", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let device_id = mkdid(pointer_id);
                let position =
                    event::pointer_position(&event).to_physical(super::scale_factor(&window));
                let kind = event::pointer_kind(&event, pointer_id);
                handler(modifiers, device_id, event.is_primary(), position, kind);
            }));
    }

    pub fn on_pointer_release<C>(&mut self, canvas_common: &Common, mut handler: C)
    where
        C: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, ButtonSource),
    {
        let window = canvas_common.window.clone();
        self.on_pointer_release =
            Some(canvas_common.add_event("pointerup", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let kind = event::pointer_kind(&event, pointer_id);
                let button = event::raw_button(&event).expect("no button pressed");

                let source = match event::pointer_source(&event, kind) {
                    PointerSource::Mouse => event::mouse_button(button),
                    PointerSource::Touch { finger_id, force } => {
                        ButtonSource::Touch { finger_id, force }
                    },
                    PointerSource::TabletTool { kind, data } => {
                        ButtonSource::TabletTool { kind, button: event::tool_button(button), data }
                    },
                    PointerSource::Unknown => ButtonSource::Unknown(button),
                };

                handler(
                    modifiers,
                    mkdid(pointer_id),
                    event.is_primary(),
                    event::pointer_position(&event).to_physical(super::scale_factor(&window)),
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
        C: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, ButtonSource),
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
                let kind = event::pointer_kind(&event, pointer_id);
                let button = event::raw_button(&event).expect("no button pressed");

                let source = match event::pointer_source(&event, kind) {
                    PointerSource::Mouse => {
                        // Error is swallowed here since the error would occur every time the
                        // mouse is clicked when the cursor is
                        // grabbed, and there is probably not a
                        // situation where this could fail, that we
                        // care if it fails.
                        let _e = canvas.set_pointer_capture(pointer_id);

                        event::mouse_button(button)
                    },
                    PointerSource::Touch { finger_id, force } => {
                        ButtonSource::Touch { finger_id, force }
                    },
                    PointerSource::TabletTool { kind, data } => {
                        // Error is swallowed here since the error would occur every time the
                        // mouse is clicked when the cursor is
                        // grabbed, and there is probably not a
                        // situation where this could fail, that we
                        // care if it fails.
                        let _e = canvas.set_pointer_capture(pointer_id);

                        ButtonSource::TabletTool { kind, button: event::tool_button(button), data }
                    },
                    PointerSource::Unknown => ButtonSource::Unknown(button),
                };

                handler(
                    modifiers,
                    mkdid(pointer_id),
                    event.is_primary(),
                    event::pointer_position(&event).to_physical(super::scale_factor(&window)),
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
                &mut dyn Iterator<
                    Item = (ModifiersState, bool, PhysicalPosition<f64>, PointerSource),
                >,
            ),
        B: 'static
            + FnMut(
                ModifiersState,
                Option<DeviceId>,
                bool,
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
                let device_id = mkdid(pointer_id);
                let kind = event::pointer_kind(&event, pointer_id);
                let primary = event.is_primary();

                // chorded button event
                if let Some(button) = event::raw_button(&event) {
                    if prevent_default.get() {
                        // prevent text selection
                        event.prevent_default();
                        // but still focus element
                        let _ = canvas.focus();
                    }

                    let state = if event::pointer_buttons(&event)
                        .contains(ButtonsState::from_bits_retain(button))
                    {
                        ElementState::Pressed
                    } else {
                        ElementState::Released
                    };

                    let button = match event::pointer_source(&event, kind) {
                        PointerSource::Mouse => event::mouse_button(button),
                        PointerSource::Touch { finger_id, force } => {
                            if button != 0 {
                                tracing::error!("unexpected touch button id: {button}");
                            }

                            ButtonSource::Touch { finger_id, force }
                        },
                        PointerSource::TabletTool { kind, data } => ButtonSource::TabletTool {
                            kind,
                            button: event::tool_button(button),
                            data,
                        },
                        PointerSource::Unknown => ButtonSource::Unknown(button),
                    };

                    button_handler(
                        event::mouse_modifiers(&event),
                        device_id,
                        primary,
                        event::pointer_position(&event).to_physical(super::scale_factor(&window)),
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
                            event.is_primary(),
                            event::pointer_position(&event).to_physical(scale),
                            event::pointer_source(&event, kind),
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
