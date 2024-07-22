use std::cell::Cell;
use std::rc::Rc;

use event::ButtonsState;
use tracing::warn;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::{Event, MouseEvent, PointerEvent};

use super::canvas::Common;
use super::event;
use super::event_handle::EventListenerHandle;
use crate::dpi::PhysicalPosition;
use crate::event::{CursorButton, CursorType, Force};
use crate::keyboard::ModifiersState;

#[allow(dead_code)]
pub(super) struct PointerHandler {
    on_cursor_leave: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_cursor_enter: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_cursor_move: Option<EventListenerHandle<dyn FnMut(PointerEventExt)>>,
    on_pointer_press: Option<EventListenerHandle<dyn FnMut(PointerEventExt)>>,
    on_pointer_release: Option<EventListenerHandle<dyn FnMut(PointerEventExt)>>,
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
                let pointer_id =
                    matches!(WebPointerType::from_event(&event), Some(WebPointerType::Mouse))
                        .then(|| event.pointer_id());

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
                let pointer_id =
                    matches!(WebPointerType::from_event(&event), Some(WebPointerType::Mouse))
                        .then(|| event.pointer_id());

                handler(modifiers, pointer_id);
            }));
    }

    pub fn on_mouse_release<MOD, C, T>(
        &mut self,
        canvas_common: &Common,
        mut modifier_handler: MOD,
        mut cursor_handler: C,
        mut touch_handler: T,
    ) where
        MOD: 'static + FnMut(ModifiersState),
        C: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, CursorType, CursorButton),
        T: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, Force),
    {
        let window = canvas_common.window.clone();
        self.on_pointer_release =
            Some(canvas_common.add_event("pointerup", move |event: PointerEventExt| {
                let modifiers = event::mouse_modifiers(&event);
                let Some(r#type) = WebPointerType::from_event(&event) else {
                    modifier_handler(modifiers);
                    return;
                };

                match r#type {
                    WebPointerType::Mouse | WebPointerType::Pen => {
                        let button = event::cursor_button(&event, r#type);
                        let r#type = event::cursor_type(&event, r#type, button.as_ref());
                        cursor_handler(
                            modifiers,
                            event.pointer_id(),
                            event::cursor_position(&event)
                                .to_physical(super::scale_factor(&window)),
                            r#type,
                            button.expect("no cursor button released"),
                        )
                    },
                    WebPointerType::Touch => touch_handler(
                        modifiers,
                        event.pointer_id(),
                        event::cursor_position(&event).to_physical(super::scale_factor(&window)),
                        Force::Normalized(event.pressure() as f64),
                    ),
                }
            }));
    }

    pub fn on_mouse_press<MOD, C, T>(
        &mut self,
        canvas_common: &Common,
        mut modifier_handler: MOD,
        mut cursor_handler: C,
        mut touch_handler: T,
        prevent_default: Rc<Cell<bool>>,
    ) where
        MOD: 'static + FnMut(ModifiersState),
        C: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, CursorType, CursorButton),
        T: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, Force),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_pointer_press =
            Some(canvas_common.add_event("pointerdown", move |event: PointerEventExt| {
                if prevent_default.get() {
                    // prevent text selection
                    event.prevent_default();
                    // but still focus element
                    let _ = canvas.focus();
                }

                let modifiers = event::mouse_modifiers(&event);
                let Some(r#type) = WebPointerType::from_event(&event) else {
                    modifier_handler(modifiers);
                    return;
                };

                match r#type {
                    WebPointerType::Mouse | WebPointerType::Pen => {
                        // Error is swallowed here since the error would occur every time the mouse
                        // is clicked when the cursor is grabbed, and there
                        // is probably not a situation where this could
                        // fail, that we care if it fails.
                        let _e = canvas.set_pointer_capture(event.pointer_id());

                        let button = event::cursor_button(&event, r#type);
                        let r#type = event::cursor_type(&event, r#type, button.as_ref());
                        cursor_handler(
                            modifiers,
                            event.pointer_id(),
                            event::cursor_position(&event)
                                .to_physical(super::scale_factor(&window)),
                            r#type,
                            button.expect("no cursor button released"),
                        )
                    },
                    WebPointerType::Touch => touch_handler(
                        modifiers,
                        event.pointer_id(),
                        event::cursor_position(&event).to_physical(super::scale_factor(&window)),
                        Force::Normalized(event.pressure() as f64),
                    ),
                }
            }));
    }

    pub fn on_cursor_move<MOD, C, T, B>(
        &mut self,
        canvas_common: &Common,
        mut modifier_handler: MOD,
        mut cursor_handler: C,
        mut touch_handler: T,
        mut button_handler: B,
        prevent_default: Rc<Cell<bool>>,
    ) where
        MOD: 'static + FnMut(ModifiersState),
        C: 'static
            + FnMut(
                ModifiersState,
                i32,
                &mut dyn Iterator<Item = (PhysicalPosition<f64>, CursorType)>,
            ),
        T: 'static
            + FnMut(ModifiersState, i32, &mut dyn Iterator<Item = (PhysicalPosition<f64>, Force)>),
        B: 'static
            + FnMut(
                ModifiersState,
                i32,
                PhysicalPosition<f64>,
                CursorType,
                ButtonsState,
                CursorButton,
            ),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_cursor_move =
            Some(canvas_common.add_event("pointermove", move |event: PointerEventExt| {
                let modifiers = event::mouse_modifiers(&event);
                let Some(r#type) = WebPointerType::from_event(&event) else {
                    modifier_handler(modifiers);
                    return;
                };

                let id = event.pointer_id();

                match r#type {
                    WebPointerType::Mouse | WebPointerType::Pen => {
                        let button = event::cursor_button(&event, r#type);

                        // chorded button event
                        if let Some(button) = button {
                            if prevent_default.get() {
                                // prevent text selection
                                event.prevent_default();
                                // but still focus element
                                let _ = canvas.focus();
                            }

                            let r#type = event::cursor_type(&event, r#type, Some(&button));
                            button_handler(
                                modifiers,
                                id,
                                event::cursor_position(&event)
                                    .to_physical(super::scale_factor(&window)),
                                r#type,
                                event::cursor_buttons(&event),
                                button,
                            );
                        } else {
                            cursor_handler(
                                modifiers,
                                id,
                                &mut event::pointer_move_event(event).map(|event| {
                                    let position = event::cursor_position(&event)
                                        .to_physical(super::scale_factor(&window));
                                    let r#type =
                                        event::cursor_type(&event, r#type, button.as_ref());

                                    (position, r#type)
                                }),
                            )
                        }
                    },
                    WebPointerType::Touch => {
                        debug_assert_eq!(
                            event::raw_button(&event),
                            None,
                            "expect pointer type of a chorded button event to be mouse or pen"
                        );

                        touch_handler(
                            modifiers,
                            id,
                            &mut event::pointer_move_event(event).map(|event| {
                                let position = event::cursor_position(&event)
                                    .to_physical(super::scale_factor(&window));
                                let pressure = event.pressure().into();

                                (position, Force::Normalized(pressure))
                            }),
                        )
                    },
                }
            }));
    }

    pub fn on_touch_cancel<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let window = canvas_common.window.clone();
        self.on_touch_cancel =
            Some(canvas_common.add_event("pointercancel", move |event: PointerEvent| {
                if matches!(WebPointerType::from_event(&event), Some(WebPointerType::Touch)) {
                    handler(
                        event.pointer_id(),
                        event::cursor_position(&event).to_physical(super::scale_factor(&window)),
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

#[derive(Clone, Copy)]
pub enum WebPointerType {
    Mouse,
    Touch,
    Pen,
}

impl WebPointerType {
    pub fn from_event(event: &PointerEvent) -> Option<Self> {
        #[allow(clippy::disallowed_methods)]
        let r#type = event.pointer_type();

        match r#type.as_ref() {
            "mouse" => Some(Self::Mouse),
            "touch" => Some(Self::Touch),
            "pen" => Some(Self::Pen),
            r#type => {
                warn!("found unknown pointer typ: {type}");
                None
            },
        }
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = PointerEvent, extends = MouseEvent, extends = Event)]
    pub type PointerEventExt;

    #[wasm_bindgen(method, getter, js_name = altitudeAngle)]
    pub fn altitude_angle(this: &PointerEventExt) -> Option<f64>;

    #[wasm_bindgen(method, getter, js_name = azimuthAngle)]
    pub fn azimuth_angle(this: &PointerEventExt) -> f64;
}
