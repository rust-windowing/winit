use super::event;
use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::error::OsError as RootOE;
use crate::event::{ModifiersState, MouseButton, MouseScrollDelta, ScanCode, VirtualKeyCode};
use crate::platform_impl::{OsError, PlatformSpecificWindowBuilderAttributes};

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{
    AddEventListenerOptions, Event, EventTarget, FocusEvent, HtmlCanvasElement, KeyboardEvent,
    MediaQueryListEvent, MouseEvent, PointerEvent, WheelEvent,
};

pub struct Canvas {
    /// Note: resizing the HTMLCanvasElement should go through `backend::set_canvas_size` to ensure the DPI factor is maintained.
    raw: HtmlCanvasElement,
    on_focus: Option<Closure<dyn FnMut(FocusEvent)>>,
    on_blur: Option<Closure<dyn FnMut(FocusEvent)>>,
    on_keyboard_release: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_keyboard_press: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_received_character: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_cursor_leave: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_cursor_enter: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_cursor_move: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_pointer_press: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_pointer_release: Option<Closure<dyn FnMut(PointerEvent)>>,
    // Fallback events when pointer event support is missing
    on_mouse_leave: Option<Closure<dyn FnMut(MouseEvent)>>,
    on_mouse_enter: Option<Closure<dyn FnMut(MouseEvent)>>,
    on_mouse_move: Option<Closure<dyn FnMut(MouseEvent)>>,
    on_mouse_press: Option<Closure<dyn FnMut(MouseEvent)>>,
    on_mouse_release: Option<Closure<dyn FnMut(MouseEvent)>>,
    on_mouse_wheel: Option<Closure<dyn FnMut(WheelEvent)>>,
    on_fullscreen_change: Option<Closure<dyn FnMut(Event)>>,
    wants_fullscreen: Rc<RefCell<bool>>,
    on_dark_mode: Option<Closure<dyn FnMut(MediaQueryListEvent)>>,
    mouse_state: MouseState,
}

impl Drop for Canvas {
    fn drop(&mut self) {
        self.raw.remove();
    }
}

impl Canvas {
    pub fn create(attr: PlatformSpecificWindowBuilderAttributes) -> Result<Self, RootOE> {
        let canvas = match attr.canvas {
            Some(canvas) => canvas,
            None => {
                let window = web_sys::window()
                    .ok_or(os_error!(OsError("Failed to obtain window".to_owned())))?;

                let document = window
                    .document()
                    .ok_or(os_error!(OsError("Failed to obtain document".to_owned())))?;

                document
                    .create_element("canvas")
                    .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?
                    .unchecked_into()
            }
        };

        // A tabindex is needed in order to capture local keyboard events.
        // A "0" value means that the element should be focusable in
        // sequential keyboard navigation, but its order is defined by the
        // document's source order.
        // https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/tabindex
        canvas
            .set_attribute("tabindex", "0")
            .map_err(|_| os_error!(OsError("Failed to set a tabindex".to_owned())))?;

        let mouse_state = if has_pointer_event() {
            MouseState::HasPointerEvent
        } else {
            MouseState::NoPointerEvent {
                on_mouse_leave_handler: Rc::new(RefCell::new(None)),
                mouse_capture_state: Rc::new(RefCell::new(MouseCaptureState::NotCaptured)),
            }
        };

        Ok(Canvas {
            raw: canvas,
            on_blur: None,
            on_focus: None,
            on_keyboard_release: None,
            on_keyboard_press: None,
            on_received_character: None,
            on_cursor_leave: None,
            on_cursor_enter: None,
            on_cursor_move: None,
            on_pointer_release: None,
            on_pointer_press: None,
            on_mouse_leave: None,
            on_mouse_enter: None,
            on_mouse_move: None,
            on_mouse_press: None,
            on_mouse_release: None,
            on_mouse_wheel: None,
            on_fullscreen_change: None,
            wants_fullscreen: Rc::new(RefCell::new(false)),
            on_dark_mode: None,
            mouse_state,
        })
    }

    pub fn set_attribute(&self, attribute: &str, value: &str) {
        self.raw
            .set_attribute(attribute, value)
            .expect(&format!("Set attribute: {}", attribute));
    }

    pub fn position(&self) -> LogicalPosition<f64> {
        let bounds = self.raw.get_bounding_client_rect();

        LogicalPosition {
            x: bounds.x(),
            y: bounds.y(),
        }
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: self.raw.width(),
            height: self.raw.height(),
        }
    }

    pub fn raw(&self) -> &HtmlCanvasElement {
        &self.raw
    }

    pub fn on_blur<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_blur = Some(self.add_event("blur", move |_: FocusEvent| {
            handler();
        }));
    }

    pub fn on_focus<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_focus = Some(self.add_event("focus", move |_: FocusEvent| {
            handler();
        }));
    }

    pub fn on_keyboard_release<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_keyboard_release =
            Some(self.add_user_event("keyup", move |event: KeyboardEvent| {
                event.prevent_default();
                handler(
                    event::scan_code(&event),
                    event::virtual_key_code(&event),
                    event::keyboard_modifiers(&event),
                );
            }));
    }

    pub fn on_keyboard_press<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_keyboard_press =
            Some(self.add_user_event("keydown", move |event: KeyboardEvent| {
                event.prevent_default();
                handler(
                    event::scan_code(&event),
                    event::virtual_key_code(&event),
                    event::keyboard_modifiers(&event),
                );
            }));
    }

    pub fn on_received_character<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(char),
    {
        // TODO: Use `beforeinput`.
        //
        // The `keypress` event is deprecated, but there does not seem to be a
        // viable/compatible alternative as of now. `beforeinput` is still widely
        // unsupported.
        self.on_received_character = Some(self.add_user_event(
            "keypress",
            move |event: KeyboardEvent| {
                handler(event::codepoint(&event));
            },
        ));
    }

    pub fn on_cursor_leave<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        match &self.mouse_state {
            MouseState::HasPointerEvent => {
                self.on_cursor_leave =
                    Some(self.add_event("pointerout", move |event: PointerEvent| {
                        handler(event.pointer_id());
                    }));
            }
            MouseState::NoPointerEvent {
                on_mouse_leave_handler,
                mouse_capture_state,
                ..
            } => {
                *on_mouse_leave_handler.borrow_mut() = Some(Box::new(handler));
                let on_mouse_leave_handler = on_mouse_leave_handler.clone();
                let mouse_capture_state = mouse_capture_state.clone();
                self.on_mouse_leave = Some(self.add_event("mouseout", move |_: MouseEvent| {
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
        }
    }

    pub fn on_cursor_enter<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        match &self.mouse_state {
            MouseState::HasPointerEvent => {
                self.on_cursor_enter =
                    Some(self.add_event("pointerover", move |event: PointerEvent| {
                        handler(event.pointer_id());
                    }));
            }
            MouseState::NoPointerEvent {
                mouse_capture_state,
                ..
            } => {
                let mouse_capture_state = mouse_capture_state.clone();
                self.on_mouse_enter = Some(self.add_event("mouseover", move |_: MouseEvent| {
                    // We don't send cursor leave events when the mouse is being
                    // captured, therefore we do the same with cursor enter events.
                    if *mouse_capture_state.borrow() != MouseCaptureState::Captured {
                        handler(0);
                    }
                }));
            }
        }
    }

    pub fn on_mouse_release<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton, ModifiersState),
    {
        match &self.mouse_state {
            MouseState::HasPointerEvent => {
                self.on_pointer_release = Some(self.add_user_event(
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
            MouseState::NoPointerEvent {
                on_mouse_leave_handler,
                mouse_capture_state,
                ..
            } => {
                let on_mouse_leave_handler = on_mouse_leave_handler.clone();
                let mouse_capture_state = mouse_capture_state.clone();
                let canvas = self.raw.clone();
                self.on_mouse_release = Some(self.add_window_mouse_event(
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
        }
    }

    pub fn on_mouse_press<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, MouseButton, ModifiersState),
    {
        match &self.mouse_state {
            MouseState::HasPointerEvent => {
                let canvas = self.raw.clone();
                self.on_pointer_press = Some(self.add_user_event(
                    "pointerdown",
                    move |event: PointerEvent| {
                        handler(
                            event.pointer_id(),
                            event::mouse_position(&event).to_physical(super::scale_factor()),
                            event::mouse_button(&event),
                            event::mouse_modifiers(&event),
                        );
                        canvas
                            .set_pointer_capture(event.pointer_id())
                            .expect("Failed to set pointer capture");
                    },
                ));
            }
            MouseState::NoPointerEvent {
                mouse_capture_state,
                ..
            } => {
                let mouse_capture_state = mouse_capture_state.clone();
                let canvas = self.raw.clone();
                self.on_mouse_press = Some(self.add_window_mouse_event(
                    "mousedown",
                    move |event: MouseEvent| {
                        let canvas = canvas.clone();
                        let mut mouse_capture_state = mouse_capture_state.borrow_mut();
                        match &*mouse_capture_state {
                            MouseCaptureState::NotCaptured
                                if event.target().map_or(false, |target| {
                                    target != EventTarget::from(canvas)
                                }) =>
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
                            event::mouse_position(&event).to_physical(super::scale_factor()),
                            event::mouse_button(&event),
                            event::mouse_modifiers(&event),
                        );
                    },
                ));
            }
        }
    }

    pub fn on_cursor_move<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, ModifiersState),
    {
        match &self.mouse_state {
            MouseState::HasPointerEvent => {
                self.on_cursor_move =
                    Some(self.add_event("pointermove", move |event: PointerEvent| {
                        handler(
                            event.pointer_id(),
                            event::mouse_position(&event).to_physical(super::scale_factor()),
                            event::mouse_modifiers(&event),
                        );
                    }));
            }
            MouseState::NoPointerEvent {
                mouse_capture_state,
                ..
            } => {
                let mouse_capture_state = mouse_capture_state.clone();
                let canvas = self.raw.clone();
                self.on_mouse_move = Some(self.add_window_mouse_event(
                    "mousemove",
                    move |event: MouseEvent| {
                        let canvas = canvas.clone();
                        let mouse_capture_state = mouse_capture_state.borrow();
                        let is_over_canvas = event
                            .target()
                            .map_or(false, |target| target == EventTarget::from(canvas.clone()));
                        match &*mouse_capture_state {
                            // Don't handle hover events outside of canvas.
                            MouseCaptureState::NotCaptured if !is_over_canvas => return,
                            MouseCaptureState::OtherElement if !is_over_canvas => return,
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
                                handler(
                                    0,
                                    mouse_pos.to_physical(super::scale_factor()),
                                    event::mouse_modifiers(&event),
                                );
                            }
                        }
                    },
                ));
            }
        }
    }

    pub fn on_mouse_wheel<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseScrollDelta, ModifiersState),
    {
        self.on_mouse_wheel = Some(self.add_event("wheel", move |event: WheelEvent| {
            event.prevent_default();
            if let Some(delta) = event::mouse_scroll_delta(&event) {
                handler(0, delta, event::mouse_modifiers(&event));
            }
        }));
    }

    pub fn on_fullscreen_change<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_fullscreen_change =
            Some(self.add_event("fullscreenchange", move |_: Event| handler()));
    }

    pub fn on_dark_mode<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(bool),
    {
        let window = web_sys::window().expect("Failed to obtain window");

        self.on_dark_mode = window
            .match_media("(prefers-color-scheme: dark)")
            .ok()
            .flatten()
            .and_then(|media| {
                let closure = Closure::wrap(Box::new(move |event: MediaQueryListEvent| {
                    handler(event.matches())
                }) as Box<dyn FnMut(_)>);

                media
                    .add_listener_with_opt_callback(Some(&closure.as_ref().unchecked_ref()))
                    .map(|_| closure)
                    .ok()
            });
    }

    fn add_event<E, F>(&self, event_name: &str, mut handler: F) -> Closure<dyn FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        let closure = Closure::wrap(Box::new(move |event: E| {
            {
                let event_ref = event.as_ref();
                event_ref.stop_propagation();
                event_ref.cancel_bubble();
            }

            handler(event);
        }) as Box<dyn FnMut(E)>);

        self.raw
            .add_event_listener_with_callback(event_name, &closure.as_ref().unchecked_ref())
            .expect("Failed to add event listener with callback");

        closure
    }

    // The difference between add_event and add_user_event is that the latter has a special meaning
    // for browser security. A user event is a deliberate action by the user (like a mouse or key
    // press) and is the only time things like a fullscreen request may be successfully completed.)
    fn add_user_event<E, F>(&self, event_name: &str, mut handler: F) -> Closure<dyn FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        let wants_fullscreen = self.wants_fullscreen.clone();
        let canvas = self.raw.clone();

        self.add_event(event_name, move |event: E| {
            handler(event);

            if *wants_fullscreen.borrow() {
                canvas
                    .request_fullscreen()
                    .expect("Failed to enter fullscreen");
                *wants_fullscreen.borrow_mut() = false;
            }
        })
    }

    // This function is used exclusively for mouse events (not pointer events).
    // Due to the need for mouse capturing, the mouse event handlers are added
    // to the window instead of the canvas element, which requires special
    // handling to control event propagation.
    fn add_window_mouse_event<F>(
        &self,
        event_name: &str,
        mut handler: F,
    ) -> Closure<dyn FnMut(MouseEvent)>
    where
        F: 'static + FnMut(MouseEvent),
    {
        let wants_fullscreen = self.wants_fullscreen.clone();
        let canvas = self.raw.clone();
        let window = web_sys::window().expect("Failed to obtain window");

        let closure = Closure::wrap(Box::new(move |event: MouseEvent| {
            handler(event);

            if *wants_fullscreen.borrow() {
                canvas
                    .request_fullscreen()
                    .expect("Failed to enter fullscreen");
                *wants_fullscreen.borrow_mut() = false;
            }
        }) as Box<dyn FnMut(_)>);

        window
            .add_event_listener_with_callback_and_add_event_listener_options(
                event_name,
                &closure.as_ref().unchecked_ref(),
                AddEventListenerOptions::new().capture(true),
            )
            .expect("Failed to add event listener with callback and options");

        closure
    }

    pub fn request_fullscreen(&self) {
        *self.wants_fullscreen.borrow_mut() = true;
    }

    pub fn is_fullscreen(&self) -> bool {
        super::is_fullscreen(&self.raw)
    }
}

enum MouseState {
    HasPointerEvent,
    NoPointerEvent {
        on_mouse_leave_handler: Rc<RefCell<Option<Box<dyn FnMut(i32)>>>>,
        mouse_capture_state: Rc<RefCell<MouseCaptureState>>,
    },
}

#[derive(PartialEq, Eq)]
enum MouseCaptureState {
    NotCaptured,
    Captured,
    OtherElement,
}

/// Returns whether pointer events are supported.
/// Used to decide whether to use pointer events
/// or plain mouse events. Note that Safari
/// doesn't support pointer events now.
fn has_pointer_event() -> bool {
    if let Some(window) = web_sys::window() {
        window.get("PointerEvent").is_some()
    } else {
        false
    }
}
