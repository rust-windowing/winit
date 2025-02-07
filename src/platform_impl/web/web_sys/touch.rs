use super::canvas::Common;
use super::event_handle::EventListenerHandle;
use crate::event::{ButtonSource, DeviceId, FingerId, Force, PointerKind, PointerSource};
use dpi::{LogicalPosition, PhysicalPosition};
use std::cell::Cell;
use std::rc::Rc;
use web_sys::{Touch, TouchEvent};

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
    where T: 'static + FnMut(Finger),
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
            Some(canvas_common.add_event("touchdown", move |event: TouchEvent| {
                if prevent_default.get() {
                    // prevent text selection
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
        let canvas = canvas_common.raw().clone();
        self.on_touch_move =
            Some(canvas_common.add_event("touchmove", move |event: TouchEvent| {
                if prevent_default.get() {
                    // prevent text selection
                    event.prevent_default();
                    // but still focus element
                    let _ = canvas.focus();
                }
                for finger in ChangedTouches::new(event, &window) {
                    handler(finger)
                }
            }));
    }

    pub fn on_touch_cancel<T>(
        &mut self,
        canvas_common: &Common,
        mut handler: T,
        prevent_default: Rc<Cell<bool>>,
    ) where
        T: 'static + FnMut(Finger),
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

pub(crate) struct Finger {
    pub position: PhysicalPosition<f64>,
    pub finger_id: FingerId,
    pub force: Option<Force>,
    pub primary: bool,
    pub device_id: Option<DeviceId>,
}

impl Finger {
    pub(crate) fn from(touch: Touch, window: &web_sys::Window) -> Self {
        let position = LogicalPosition::new(
                touch.client_x() as f64,
                touch.client_y() as f64,
            ).to_physical(super::scale_factor(window));
        let finger_id = FingerId(touch.identifier() as usize);
        let force = Some(Force::Normalized(touch.force() as f64));

        Self {
            position,
            finger_id,
            force,
            primary: finger_id.0 == 0, // TODO: is there a more accurate way to get this?
            device_id: None, // TODO: how to get device ID?
        }
    }
    
    pub(crate) fn pointer_source(&self) -> PointerSource {
        PointerSource::Touch {
            finger_id: self.finger_id,
            force: self.force,
        }
    }
    
    pub(crate) fn button_source(&self) -> ButtonSource {
        ButtonSource::Touch {
            finger_id: self.finger_id,
            force: self.force,
        }
    }
    
    pub(crate) fn pointer_kind(&self) -> PointerKind {
        PointerKind::Touch(self.finger_id)
    }
}

struct ChangedTouches {
    index: u32,
    event: TouchEvent,
    window: web_sys::Window,
}

impl ChangedTouches {
    pub(super) fn new<'a>(event: TouchEvent, window: &web_sys::Window) -> Self {
        Self {
            index: 0,
            event,
            window: window.clone(),
        }
    }
}

impl Iterator for ChangedTouches {
    type Item = Finger;

    fn next(&mut self) -> Option<Self::Item> {
        match self.event.changed_touches().get(self.index) {
            None => None,
            Some(touch) => {
                self.index += 1;
                Some(Finger::from(touch, &self.window))
            }
        }
    }
}