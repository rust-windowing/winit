use super::event;
use super::EventListenerHandle;
use crate::dpi::PhysicalPosition;
use crate::event::{ModifiersState, MouseButton};

use std::cell::RefCell;
use std::rc::Rc;

use web_sys::{EventTarget, MouseEvent};

type MouseLeaveHandler = Rc<RefCell<Option<Box<dyn FnMut(i32)>>>>;

#[allow(dead_code)]
pub(super) struct MouseHandler {
    on_mouse_leave: Option<EventListenerHandle<dyn FnMut(MouseEvent)>>,
    on_mouse_enter: Option<EventListenerHandle<dyn FnMut(MouseEvent)>>,
    on_mouse_move: Option<EventListenerHandle<dyn FnMut(MouseEvent)>>,
    on_mouse_press: Option<EventListenerHandle<dyn FnMut(MouseEvent)>>,
    on_mouse_release: Option<EventListenerHandle<dyn FnMut(MouseEvent)>>,
    on_mouse_leave_handler: MouseLeaveHandler,
    mouse_capture_state: Rc<RefCell<MouseCaptureState>>,
}

#[derive(PartialEq, Eq)]
pub(super) enum MouseCaptureState {
    NotCaptured,
    Captured,
    OtherElement,
}

impl MouseHandler {
    pub fn new() -> Self {
        Self {
            on_mouse_leave: None,
            on_mouse_enter: None,
            on_mouse_move: None,
            on_mouse_press: None,
            on_mouse_release: None,
            on_mouse_leave_handler: Rc::new(RefCell::new(None)),
            mouse_capture_state: Rc::new(RefCell::new(MouseCaptureState::NotCaptured)),
        }
    }
    pub fn on_cursor_leave<F>(&mut self, canvas_common: &super::Common, handler: F)
    where
        F: 'static + FnMut(i32),
    {
        *self.on_mouse_leave_handler.borrow_mut() = Some(Box::new(handler));
        let on_mouse_leave_handler = self.on_mouse_leave_handler.clone();
        let mouse_capture_state = self.mouse_capture_state.clone();
        self.on_mouse_leave = Some(canvas_common.add_event("mouseout", move |_: MouseEvent| {
            // If the mouse is being captured, it is always considered
            // to be "within" the the canvas, until the capture has been
            // released, therefore we don't send cursor leave events.
            if *mouse_capture_state.borrow() != MouseCaptureState::Captured {
                if let Some(handler) = on_mouse_leave_handler.borrow_mut().as_mut() {
                    handler(0);
                }
            }
        }));
    }

    pub fn on_cursor_enter<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        let mouse_capture_state = self.mouse_capture_state.clone();
        self.on_mouse_enter = Some(canvas_common.add_event("mouseover", move |_: MouseEvent| {
            // We don't send cursor leave events when the mouse is being
            // captured, therefore we do the same with cursor enter events.
            if *mouse_capture_state.borrow() != MouseCaptureState::Captured {
                handler(0);
            }
        }));
    }

    pub fn on_mouse_release<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton, ModifiersState),
    {
        let on_mouse_leave_handler = self.on_mouse_leave_handler.clone();
        let mouse_capture_state = self.mouse_capture_state.clone();
        let canvas = canvas_common.raw.clone();
        self.on_mouse_release = Some(canvas_common.add_window_mouse_event(
            "mouseup",
            move |event: MouseEvent| {
                let canvas = canvas.clone();
                let mut mouse_capture_state = mouse_capture_state.borrow_mut();
                match &*mouse_capture_state {
                    // Shouldn't happen but we'll just ignore it.
                    MouseCaptureState::NotCaptured => return,
                    MouseCaptureState::OtherElement => {
                        if event.buttons() == 0 {
                            // No buttons are pressed anymore so reset
                            // the capturing state.
                            *mouse_capture_state = MouseCaptureState::NotCaptured;
                        }
                        return;
                    }
                    MouseCaptureState::Captured => {}
                }
                event.stop_propagation();
                handler(
                    0,
                    event::mouse_button(&event),
                    event::mouse_modifiers(&event),
                );
                if event
                    .target()
                    .map_or(false, |target| target != EventTarget::from(canvas))
                {
                    // Since we do not send cursor leave events while the
                    // cursor is being captured, we instead send it after
                    // the capture has been released.
                    if let Some(handler) = on_mouse_leave_handler.borrow_mut().as_mut() {
                        handler(0);
                    }
                }
                if event.buttons() == 0 {
                    // No buttons are pressed anymore so reset
                    // the capturing state.
                    *mouse_capture_state = MouseCaptureState::NotCaptured;
                }
            },
        ));
    }

    pub fn on_mouse_press<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, MouseButton, ModifiersState),
    {
        let mouse_capture_state = self.mouse_capture_state.clone();
        let canvas = canvas_common.raw.clone();
        self.on_mouse_press = Some(canvas_common.add_window_mouse_event(
            "mousedown",
            move |event: MouseEvent| {
                let canvas = canvas.clone();
                let mut mouse_capture_state = mouse_capture_state.borrow_mut();
                match &*mouse_capture_state {
                    MouseCaptureState::NotCaptured
                        if event
                            .target()
                            .map_or(false, |target| target != EventTarget::from(canvas)) =>
                    {
                        // The target isn't our canvas which means the
                        // mouse is pressed outside of it.
                        *mouse_capture_state = MouseCaptureState::OtherElement;
                        return;
                    }
                    MouseCaptureState::OtherElement => return,
                    _ => {}
                }
                *mouse_capture_state = MouseCaptureState::Captured;
                event.stop_propagation();
                handler(
                    0,
                    event::mouse_position(&event).to_physical(super::super::scale_factor()),
                    event::mouse_button(&event),
                    event::mouse_modifiers(&event),
                );
            },
        ));
    }

    pub fn on_cursor_move<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, PhysicalPosition<f64>, ModifiersState),
    {
        let mouse_capture_state = self.mouse_capture_state.clone();
        let canvas = canvas_common.raw.clone();
        self.on_mouse_move = Some(canvas_common.add_window_mouse_event(
            "mousemove",
            move |event: MouseEvent| {
                let canvas = canvas.clone();
                let mouse_capture_state = mouse_capture_state.borrow();
                let is_over_canvas = event
                    .target()
                    .map_or(false, |target| target == EventTarget::from(canvas.clone()));
                match &*mouse_capture_state {
                    // Don't handle hover events outside of canvas.
                    MouseCaptureState::NotCaptured | MouseCaptureState::OtherElement
                        if !is_over_canvas => {}
                    // If hovering over the canvas, just send the cursor move event.
                    MouseCaptureState::NotCaptured
                    | MouseCaptureState::OtherElement
                    | MouseCaptureState::Captured => {
                        if *mouse_capture_state == MouseCaptureState::Captured {
                            event.stop_propagation();
                        }
                        let mouse_pos = if is_over_canvas {
                            event::mouse_position(&event)
                        } else {
                            // Since the mouse is not on the canvas, we cannot
                            // use `offsetX`/`offsetY`.
                            event::mouse_position_by_client(&event, &canvas)
                        };
                        let mouse_delta = event::mouse_delta(&event);
                        handler(
                            0,
                            mouse_pos.to_physical(super::super::scale_factor()),
                            mouse_delta.to_physical(super::super::scale_factor()),
                            event::mouse_modifiers(&event),
                        );
                    }
                }
            },
        ));
    }

    pub fn remove_listeners(&mut self) {
        self.on_mouse_leave = None;
        self.on_mouse_enter = None;
        self.on_mouse_move = None;
        self.on_mouse_press = None;
        self.on_mouse_release = None;
        *self.on_mouse_leave_handler.borrow_mut() = None;
    }
}
