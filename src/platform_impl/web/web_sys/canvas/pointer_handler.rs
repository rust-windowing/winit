use super::event;
use super::EventListenerHandle;
use crate::dpi::PhysicalPosition;
use crate::event::Force;
use crate::event::{ModifiersState, MouseButton};

use web_sys::HtmlCanvasElement;
use web_sys::PointerEvent;

#[allow(dead_code)]
pub(super) struct PointerHandler {
    on_cursor_leave: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_cursor_enter: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_cursor_move: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_pointer_press: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_pointer_release: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_touch_move: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_touch_down: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_touch_up: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
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
            on_touch_move: None,
            on_touch_down: None,
            on_touch_up: None,
            on_touch_cancel: None,
        }
    }

    pub fn on_cursor_leave<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        self.on_cursor_leave = Some(canvas_common.add_event(
            "pointerout",
            move |event: PointerEvent| {
                handler(event.pointer_id());
            },
        ));
    }

    pub fn on_cursor_enter<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        self.on_cursor_enter = Some(canvas_common.add_event(
            "pointerover",
            move |event: PointerEvent| {
                handler(event.pointer_id());
            },
        ));
    }

    pub fn on_mouse_release<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton, ModifiersState),
    {
        self.on_pointer_release = Some(canvas_common.add_user_event(
            "pointerup",
            move |event: PointerEvent| {
                handler(
                    event.pointer_id(),
                    event::mouse_button(&event),
                    event::mouse_modifiers(&event),
                );
            },
        ));
    }

    pub fn on_mouse_press<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, MouseButton, ModifiersState),
    {
        let canvas = canvas_common.raw.clone();
        self.on_pointer_press = Some(canvas_common.add_user_event(
            "pointerdown",
            move |event: PointerEvent| {
                handler(
                    event.pointer_id(),
                    event::mouse_position(&event).to_physical(super::super::scale_factor()),
                    event::mouse_button(&event),
                    event::mouse_modifiers(&event),
                );

                // Error is swallowed here since the error would occur every time the mouse is
                // clicked when the cursor is grabbed, and there is probably not a situation where
                // this could fail, that we care if it fails.
                let _e = canvas.set_pointer_capture(event.pointer_id());
            },
        ));
    }

    pub fn on_cursor_move<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, PhysicalPosition<f64>, ModifiersState),
    {
        self.on_cursor_move = Some(canvas_common.add_event(
            "pointermove",
            move |event: PointerEvent| {
                handler(
                    event.pointer_id(),
                    event::mouse_position(&event).to_physical(super::super::scale_factor()),
                    event::mouse_delta(&event).to_physical(super::super::scale_factor()),
                    event::mouse_modifiers(&event),
                );
            },
        ));
    }

    pub fn on_touch_move<F>(&mut self, canvas_common: &super::Common, handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        self.on_touch_move =
            Some(canvas_common.add_event("pointermove", touch_handler(handler, canvas_common)));
    }

    pub fn on_touch_down<F>(&mut self, canvas_common: &super::Common, handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        self.on_touch_down =
            Some(canvas_common.add_event("pointerdown", touch_handler(handler, canvas_common)));
    }

    pub fn on_touch_up<F>(&mut self, canvas_common: &super::Common, handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        self.on_touch_up =
            Some(canvas_common.add_event("pointerup", touch_handler(handler, canvas_common)));
    }

    pub fn on_touch_cancel<F>(&mut self, canvas_common: &super::Common, handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        self.on_touch_cancel =
            Some(canvas_common.add_event("pointercancel", touch_handler(handler, canvas_common)));
    }

    pub fn remove_listeners(&mut self) {
        self.on_cursor_leave = None;
        self.on_cursor_enter = None;
        self.on_cursor_move = None;
        self.on_pointer_press = None;
        self.on_pointer_release = None;
        self.on_touch_move = None;
        self.on_touch_down = None;
        self.on_touch_up = None;
        self.on_touch_cancel = None;
    }
}

fn touch_handler<F>(mut handler: F, canvas_common: &super::Common) -> impl FnMut(PointerEvent)
where
    F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
{
    let canvas = canvas_common.raw.clone();
    move |event: PointerEvent| {
        if event.pointer_type() != "touch" {
            return;
        }

        handler(
            event.pointer_id(),
            touch_physical_position(&event, &canvas),
            Force::Normalized(event.pressure() as f64),
        );
    }
}

fn touch_physical_position(
    event: &PointerEvent,
    canvas: &HtmlCanvasElement,
) -> PhysicalPosition<f64> {
    // Calculate the scale factor every time, since it's possible for the canvas's width and height attributes to get out of sync with its actual HTML size.
    let dpi_width = canvas.width() as f64 / canvas.offset_width() as f64;
    let dpi_height = canvas.height() as f64 / canvas.offset_height() as f64;
    PhysicalPosition {
        x: event.client_x() as f64 * dpi_width,
        // Flip the Y axis because canvas's origin is top-left.
        y: canvas.height() as f64 - event.client_y() as f64 * dpi_height,
    }
}
