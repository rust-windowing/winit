use std::cell::Cell;
use std::rc::Rc;

use super::canvas::Common;
use super::event;
use super::event_handle::EventListenerHandle;
use crate::dpi::PhysicalPosition;
use crate::event::{Force, MouseButton};
use crate::keyboard::ModifiersState;

use event::ButtonsState;
use web_sys::PointerEvent;

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

    pub fn on_cursor_leave<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static + FnMut(ModifiersState, Option<i32>),
    {
        self.on_cursor_leave =
            Some(canvas_common.add_event("pointerout", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);

                // touch events are handled separately
                // handling them here would produce duplicate mouse events, inconsistent with
                // other platforms.
                let pointer_id = (event.pointer_type() != "touch").then(|| event.pointer_id());

                handler(modifiers, pointer_id);
            }));
    }

    pub fn on_cursor_enter<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static + FnMut(ModifiersState, Option<i32>),
    {
        self.on_cursor_enter =
            Some(canvas_common.add_event("pointerover", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);

                // touch events are handled separately
                // handling them here would produce duplicate mouse events, inconsistent with
                // other platforms.
                let pointer_id = (event.pointer_type() != "touch").then(|| event.pointer_id());

                handler(modifiers, pointer_id);
            }));
    }

    pub fn on_mouse_release<M, T>(
        &mut self,
        canvas_common: &Common,
        mut mouse_handler: M,
        mut touch_handler: T,
    ) where
        M: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, MouseButton),
        T: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, Force),
    {
        let window = canvas_common.window.clone();
        self.on_pointer_release =
            Some(canvas_common.add_event("pointerup", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);

                match event.pointer_type().as_str() {
                    "touch" => touch_handler(
                        modifiers,
                        event.pointer_id(),
                        event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                        Force::Normalized(event.pressure() as f64),
                    ),
                    _ => mouse_handler(
                        modifiers,
                        event.pointer_id(),
                        event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                        event::mouse_button(&event).expect("no mouse button released"),
                    ),
                }
            }));
    }

    pub fn on_mouse_press<M, T>(
        &mut self,
        canvas_common: &Common,
        mut mouse_handler: M,
        mut touch_handler: T,
        prevent_default: Rc<Cell<bool>>,
    ) where
        M: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, MouseButton),
        T: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, Force),
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
                let pointer_type = &event.pointer_type();

                match pointer_type.as_str() {
                    "touch" => {
                        touch_handler(
                            modifiers,
                            event.pointer_id(),
                            event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                            Force::Normalized(event.pressure() as f64),
                        );
                    },
                    _ => {
                        mouse_handler(
                            modifiers,
                            event.pointer_id(),
                            event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                            event::mouse_button(&event).expect("no mouse button pressed"),
                        );

                        if pointer_type == "mouse" {
                            // Error is swallowed here since the error would occur every time the
                            // mouse is clicked when the cursor is
                            // grabbed, and there is probably not a
                            // situation where this could fail, that we
                            // care if it fails.
                            let _e = canvas.set_pointer_capture(event.pointer_id());
                        }
                    },
                }
            }));
    }

    pub fn on_cursor_move<M, T, B>(
        &mut self,
        canvas_common: &Common,
        mut mouse_handler: M,
        mut touch_handler: T,
        mut button_handler: B,
        prevent_default: Rc<Cell<bool>>,
    ) where
        M: 'static + FnMut(ModifiersState, i32, &mut dyn Iterator<Item = PhysicalPosition<f64>>),
        T: 'static
            + FnMut(ModifiersState, i32, &mut dyn Iterator<Item = (PhysicalPosition<f64>, Force)>),
        B: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, ButtonsState, MouseButton),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_cursor_move =
            Some(canvas_common.add_event("pointermove", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);

                let id = event.pointer_id();

                // chorded button event
                if let Some(button) = event::mouse_button(&event) {
                    if prevent_default.get() {
                        // prevent text selection
                        event.prevent_default();
                        // but still focus element
                        let _ = canvas.focus();
                    }

                    button_handler(
                        modifiers,
                        id,
                        event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                        event::mouse_buttons(&event),
                        button,
                    );

                    return;
                }

                // pointer move event
                let scale = super::scale_factor(&window);
                match event.pointer_type().as_str() {
                    "touch" => touch_handler(
                        modifiers,
                        id,
                        &mut event::pointer_move_event(event).map(|event| {
                            (
                                event::mouse_position(&event).to_physical(scale),
                                Force::Normalized(event.pressure() as f64),
                            )
                        }),
                    ),
                    _ => mouse_handler(
                        modifiers,
                        id,
                        &mut event::pointer_move_event(event)
                            .map(|event| event::mouse_position(&event).to_physical(scale)),
                    ),
                };
            }));
    }

    pub fn on_touch_cancel<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let window = canvas_common.window.clone();
        self.on_touch_cancel =
            Some(canvas_common.add_event("pointercancel", move |event: PointerEvent| {
                if event.pointer_type() == "touch" {
                    handler(
                        event.pointer_id(),
                        event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                        Force::Normalized(event.pressure() as f64),
                    );
                }
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
