use std::cell::Cell;
use std::cmp::min;
use std::rc::Rc;

use dpi::{LogicalPosition, PhysicalPosition};
use web_sys::{Touch, TouchEvent};

use super::canvas::Common;
use super::event_handle::EventListenerHandle;
use crate::event::{ButtonSource, DeviceId, FingerId, Force, PointerKind, PointerSource};
use crate::platform_impl::web::event::mkdid;

/// This module is responsible for handling touch events on the web. It does this by allowing the
/// caller of this code to define handlers for each of the events, which are of type
/// `T: 'static + FnMut(Finger)`.
///
/// Touch Events are defined in the W3C spec here: <https://www.w3.org/TR/touch-events>
/// I reference parts of this document in the documentation of this module.
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
        T: 'static + FnMut(Finger),
    {
        let window = canvas_common.window.clone();
        self.on_touch_end = Some(canvas_common.add_event("touchend", move |event: TouchEvent| {
            for finger in ChangedTouches::new(event, &window) {
                handler(finger)
            }
        }));
    }

    pub fn on_touch_start<T>(
        &mut self,
        canvas_common: &Common,
        mut handler: T,
        prevent_default: Rc<Cell<bool>>,
    ) where
        T: 'static + FnMut(Finger),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_touch_start =
            Some(canvas_common.add_event("touchstart", move |event: TouchEvent| {
                // From w3.org: If the preventDefault method is called on this event, it should
                // prevent any default actions caused by any touch events associated with the same
                // active touch point, including mouse events or scrolling.
                if prevent_default.get() {
                    event.prevent_default();
                    // but still focus element
                    let _ = canvas.focus();
                }
                for finger in ChangedTouches::new(event, &window) {
                    handler(finger)
                }
            }));
    }

    pub fn on_touch_move<T>(
        &mut self,
        canvas_common: &Common,
        mut handler: T,
        prevent_default: Rc<Cell<bool>>,
    ) where
        T: 'static + FnMut(Finger),
    {
        let window = canvas_common.window.clone();
        self.on_touch_move =
            Some(canvas_common.add_event("touchmove", move |event: TouchEvent| {
                // From w3.org: If the preventDefault method is called on the first touchmove event
                // of an active touch point, it should prevent any default action caused by any
                // touchmove event associated with the same active touch point, such as scrolling.
                if prevent_default.get() {
                    event.prevent_default();
                }
                for finger in ChangedTouches::new(event, &window) {
                    handler(finger)
                }
            }));
    }

    pub fn on_touch_cancel<T>(&mut self, canvas_common: &Common, mut handler: T)
    where
        T: 'static + FnMut(Finger),
    {
        let window = canvas_common.window.clone();
        self.on_touch_cancel =
            Some(canvas_common.add_event("touchcancel", move |event: TouchEvent| {
                for finger in ChangedTouches::new(event, &window) {
                    handler(finger)
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

/// Information about a finger in a touch event, converted to winit's native types.
pub struct Finger {
    pub position: PhysicalPosition<f64>,
    pub finger_id: FingerId,
    pub force: Option<Force>,
    pub primary: bool,
    pub device_id: Option<DeviceId>,
}

impl Finger {
    fn from(touch: Touch, window: &web_sys::Window, primary: bool) -> Self {
        let position = LogicalPosition::new(touch.client_x() as f64, touch.client_y() as f64)
            .to_physical(super::scale_factor(window));
        let finger_id = FingerId(touch.identifier() as usize);
        let force = Some(Force::Normalized(touch.force() as f64));

        Self {
            position,
            finger_id,
            force,
            primary,
            device_id: mkdid(touch.identifier()), // TODO: I'm not sure if this is right
        }
    }

    pub fn pointer_source(&self) -> PointerSource {
        PointerSource::Touch { finger_id: self.finger_id, force: self.force }
    }

    pub fn button_source(&self) -> ButtonSource {
        ButtonSource::Touch { finger_id: self.finger_id, force: self.force }
    }

    pub fn pointer_kind(&self) -> PointerKind {
        PointerKind::Touch(self.finger_id)
    }
}

struct ChangedTouches {
    index: u32,
    event: TouchEvent,
    window: web_sys::Window,
    min_touch_id: i32,
}

impl ChangedTouches {
    fn new(event: TouchEvent, window: &web_sys::Window) -> Self {
        // On both Chrome and Firefox, the touch IDs are sequential, so we can find which one is
        // the primary (first) touch by checking if its ID is the lowest of all the current touches:
        let mut min_touch_id = i32::MAX;
        for i in 0..event.touches().length() {
            if let Some(touch_id) = event.touches().get(i).map(|t| t.identifier()) {
                min_touch_id = min(min_touch_id, touch_id);
            }
        }
        Self { index: 0, event, window: window.clone(), min_touch_id }
    }
}

impl Iterator for ChangedTouches {
    type Item = Finger;

    fn next(&mut self) -> Option<Self::Item> {
        match self.event.changed_touches().get(self.index) {
            None => None,
            Some(touch) => {
                self.index += 1;
                let is_primary = touch.identifier() == self.min_touch_id;
                Some(Finger::from(touch, &self.window, is_primary))
            },
        }
    }
}
