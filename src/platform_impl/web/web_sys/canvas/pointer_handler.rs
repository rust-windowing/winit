use super::event;
use super::EventListenerHandle;
use crate::dpi::PhysicalPosition;
use crate::event::Force;

use web_sys::PointerEvent;

#[allow(dead_code)]
pub(super) struct PointerHandler {
    on_cursor_move: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_pointer_press: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_pointer_release: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_touch_cancel: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
}

impl PointerHandler {
    pub fn new() -> Self {
        Self {
            on_cursor_move: None,
            on_pointer_press: None,
            on_pointer_release: None,
            on_touch_cancel: None,
        }
    }

    pub fn on_mouse_release<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let canvas = canvas_common.raw.clone();
        self.on_pointer_release = Some(canvas_common.add_user_event(
            "pointerup",
            move |event: PointerEvent| {
                if event.pointer_type() == "touch" {
                    handler(
                        event.pointer_id(),
                        event::touch_position(&event, &canvas)
                            .to_physical(super::super::scale_factor()),
                        Force::Normalized(event.pressure() as f64),
                    );
                }
            },
        ));
    }

    pub fn on_mouse_press<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let canvas = canvas_common.raw.clone();
        self.on_pointer_press = Some(canvas_common.add_user_event(
            "pointerdown",
            move |event: PointerEvent| {
                if event.pointer_type() == "touch" {
                    handler(
                        event.pointer_id(),
                        event::touch_position(&event, &canvas)
                            .to_physical(super::super::scale_factor()),
                        Force::Normalized(event.pressure() as f64),
                    );
                }
            },
        ));
    }

    pub fn on_cursor_move<F>(
        &mut self,
        canvas_common: &super::Common,
        mut handler: F,
        prevent_default: bool,
    ) where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let canvas = canvas_common.raw.clone();
        self.on_cursor_move = Some(canvas_common.add_event(
            "pointermove",
            move |event: PointerEvent| {
                if event.pointer_type() == "touch" {
                    if prevent_default {
                        // prevent scroll on mobile web
                        event.prevent_default();
                    }
                    handler(
                        event.pointer_id(),
                        event::touch_position(&event, &canvas)
                            .to_physical(super::super::scale_factor()),
                        Force::Normalized(event.pressure() as f64),
                    );
                }
            },
        ));
    }

    pub fn on_touch_cancel<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let canvas = canvas_common.raw.clone();
        self.on_touch_cancel = Some(canvas_common.add_event(
            "pointercancel",
            move |event: PointerEvent| {
                if event.pointer_type() == "touch" {
                    handler(
                        event.pointer_id(),
                        event::touch_position(&event, &canvas)
                            .to_physical(super::super::scale_factor()),
                        Force::Normalized(event.pressure() as f64),
                    );
                }
            },
        ));
    }

    pub fn remove_listeners(&mut self) {
        self.on_cursor_move = None;
        self.on_pointer_press = None;
        self.on_pointer_release = None;
        self.on_touch_cancel = None;
    }
}
