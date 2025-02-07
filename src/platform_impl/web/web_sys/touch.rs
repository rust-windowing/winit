use super::canvas::Common;
use super::event;
use super::event_handle::EventListenerHandle;
use crate::dpi::PhysicalPosition;
use crate::event::{DeviceId, FingerId, Force};
use std::cell::Cell;
use std::rc::Rc;
use web_sys::TouchEvent;

#[allow(dead_code)]
pub(super) struct TouchHandler {
    on_touch_start: Option<EventListenerHandle<dyn FnMut(TouchEvent)>>,
    on_touch_end: Option<EventListenerHandle<dyn FnMut(TouchEvent)>>,
    on_touch_move: Option<EventListenerHandle<dyn FnMut(TouchEvent)>>,
    on_touch_cancel: Option<EventListenerHandle<dyn FnMut(TouchEvent)>>,
}

impl TouchHandler {
    pub fn new() -> Self {
        Self {
            on_touch_start: None,
            on_touch_end: None,
            on_touch_move: None,
            on_touch_cancel: None,
        }
    }

    pub fn on_touch_end<T>(&mut self, canvas_common: &Common, mut handler: T)
    where
        T: 'static
        + FnMut(Option<DeviceId>, bool, PhysicalPosition<f64>, FingerId),
    {
        let window = canvas_common.window.clone();
        self.on_touch_end =
            Some(canvas_common.add_event("touchend", move |event: TouchEvent| {
                let changed_touches = event::changed_touches(event);
                for touch in changed_touches {
                    handler(
                        None, // TODO: how to get device ID?
                        touch.identifier() == 0, // TODO: this is probably not an accurate way to check if it's the primary finger
                        event::finger_position(&touch).to_physical(super::scale_factor(&window)),
                        event::finger_id(&touch),
                    )
                }
            }));
    }

    pub fn on_touch_start<T>(
        &mut self,
        canvas_common: &Common,
        mut handler: T,
        prevent_default: Rc<Cell<bool>>,
    ) where
        T: 'static
        + FnMut(Option<DeviceId>, bool, PhysicalPosition<f64>, FingerId, Option<Force>),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_touch_start =
            Some(canvas_common.add_event("touchdown", move |event: TouchEvent| {
                if prevent_default.get() {
                    // prevent text selection
                    event.prevent_default();
                    // but still focus element
                    let _ = canvas.focus();
                }
                for touch in event::changed_touches(event) {
                    handler(
                        None, // TODO: how to get device ID?
                        touch.identifier() == 0, // TODO: this is probably not an accurate way to check if it's the primary finger
                        event::finger_position(&touch).to_physical(super::scale_factor(&window)),
                        event::finger_id(&touch),
                        event::finger_force(&touch),
                    )
                }
            }));
    }

    pub fn on_touch_move<T>(
        &mut self,
        canvas_common: &Common,
        mut handler: T,
        prevent_default: Rc<Cell<bool>>,
    ) where
        T: 'static
        + FnMut(Option<DeviceId>, bool, PhysicalPosition<f64>, FingerId, Option<Force>),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_touch_move =
            Some(canvas_common.add_event("touchmove", move |event: TouchEvent| {
                if prevent_default.get() {
                    // prevent text selection
                    event.prevent_default();
                    // but still focus element
                    let _ = canvas.focus();
                }
                for touch in event::changed_touches(event) {
                    handler(
                        None, // TODO: how to get device ID?
                        touch.identifier() == 0, // TODO: this is probably not an accurate way to check if it's the primary finger
                        event::finger_position(&touch).to_physical(super::scale_factor(&window)),
                        event::finger_id(&touch),
                        event::finger_force(&touch),
                    )
                }
            }));
    }
    
    pub fn on_touch_cancel<T>(
        &mut self,
        canvas_common: &Common,
        mut handler: T,
        prevent_default: Rc<Cell<bool>>,
    ) where
        T: 'static
        + FnMut(Option<DeviceId>, bool, PhysicalPosition<f64>, FingerId),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_touch_cancel =
            Some(canvas_common.add_event("touchcancel", move |event: TouchEvent| {
                if prevent_default.get() {
                    // prevent text selection
                    event.prevent_default();
                    // but still focus element
                    let _ = canvas.focus();
                }
                for touch in event::changed_touches(event) {
                    handler(
                        None, // TODO: how to get device ID?
                        touch.identifier() == 0, // TODO: this is probably not an accurate way to check if it's the primary finger
                        event::finger_position(&touch).to_physical(super::scale_factor(&window)),
                        event::finger_id(&touch),
                    )
                }
            }));
    }
    
    pub fn remove_listeners(&mut self) {
        self.on_touch_move = None;
        self.on_touch_end = None;
        self.on_touch_start = None;
        self.on_touch_cancel = None;
    }
}
