use super::event;
use super::event_handle::EventListenerHandle;
use super::media_query_handle::MediaQueryListHandle;
use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::error::OsError as RootOE;
use crate::event::{
    Force, ModifiersState, MouseButton, MouseScrollDelta, ScanCode, VirtualKeyCode,
};
use crate::platform_impl::{OsError, PlatformSpecificWindowBuilderAttributes};

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{
    AddEventListenerOptions, Event, FocusEvent, HtmlCanvasElement, KeyboardEvent,
    MediaQueryListEvent, MouseEvent, WheelEvent,
};

mod mouse_handler;
mod pointer_handler;

#[allow(dead_code)]
pub struct Canvas {
    common: Common,
    on_touch_start: Option<EventListenerHandle<dyn FnMut(Event)>>,
    on_touch_end: Option<EventListenerHandle<dyn FnMut(Event)>>,
    on_focus: Option<EventListenerHandle<dyn FnMut(FocusEvent)>>,
    on_blur: Option<EventListenerHandle<dyn FnMut(FocusEvent)>>,
    on_keyboard_release: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
    on_keyboard_press: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
    on_received_character: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
    on_mouse_wheel: Option<EventListenerHandle<dyn FnMut(WheelEvent)>>,
    on_fullscreen_change: Option<EventListenerHandle<dyn FnMut(Event)>>,
    on_dark_mode: Option<MediaQueryListHandle>,
    mouse_state: MouseState,
}

struct Common {
    /// Note: resizing the HTMLCanvasElement should go through `backend::set_canvas_size` to ensure the DPI factor is maintained.
    raw: HtmlCanvasElement,
    wants_fullscreen: Rc<RefCell<bool>>,
}

impl Canvas {
    pub fn create(attr: PlatformSpecificWindowBuilderAttributes) -> Result<Self, RootOE> {
        let canvas = match attr.canvas {
            Some(canvas) => canvas,
            None => {
                let window = web_sys::window()
                    .ok_or_else(|| os_error!(OsError("Failed to obtain window".to_owned())))?;

                let document = window
                    .document()
                    .ok_or_else(|| os_error!(OsError("Failed to obtain document".to_owned())))?;

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
        if attr.focusable {
            canvas
                .set_attribute("tabindex", "0")
                .map_err(|_| os_error!(OsError("Failed to set a tabindex".to_owned())))?;
        }

        let mouse_state = if has_pointer_event() {
            MouseState::HasPointerEvent(pointer_handler::PointerHandler::new())
        } else {
            MouseState::NoPointerEvent(mouse_handler::MouseHandler::new())
        };

        Ok(Canvas {
            common: Common {
                raw: canvas,
                wants_fullscreen: Rc::new(RefCell::new(false)),
            },
            on_touch_start: None,
            on_touch_end: None,
            on_blur: None,
            on_focus: None,
            on_keyboard_release: None,
            on_keyboard_press: None,
            on_received_character: None,
            on_mouse_wheel: None,
            on_fullscreen_change: None,
            on_dark_mode: None,
            mouse_state,
        })
    }

    pub fn set_cursor_lock(&self, lock: bool) -> Result<(), RootOE> {
        if lock {
            self.raw().request_pointer_lock();
        } else {
            let window = web_sys::window()
                .ok_or_else(|| os_error!(OsError("Failed to obtain window".to_owned())))?;
            let document = window
                .document()
                .ok_or_else(|| os_error!(OsError("Failed to obtain document".to_owned())))?;
            document.exit_pointer_lock();
        }
        Ok(())
    }

    pub fn set_attribute(&self, attribute: &str, value: &str) {
        self.common
            .raw
            .set_attribute(attribute, value)
            .unwrap_or_else(|err| panic!("error: {err:?}\nSet attribute: {attribute}"))
    }

    pub fn position(&self) -> LogicalPosition<f64> {
        let bounds = self.common.raw.get_bounding_client_rect();

        LogicalPosition {
            x: bounds.x(),
            y: bounds.y(),
        }
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: self.common.raw.width(),
            height: self.common.raw.height(),
        }
    }

    pub fn raw(&self) -> &HtmlCanvasElement {
        &self.common.raw
    }

    pub fn on_touch_start(&mut self, prevent_default: bool) {
        self.on_touch_start = Some(self.common.add_event("touchstart", move |event: Event| {
            if prevent_default {
                event.prevent_default();
            }
        }));
    }

    pub fn on_touch_end(&mut self, prevent_default: bool) {
        self.on_touch_end = Some(self.common.add_event("touchend", move |event: Event| {
            if prevent_default {
                event.prevent_default();
            }
        }));
    }

    pub fn on_blur<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_blur = Some(self.common.add_event("blur", move |_: FocusEvent| {
            handler();
        }));
    }

    pub fn on_focus<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_focus = Some(self.common.add_event("focus", move |_: FocusEvent| {
            handler();
        }));
    }

    pub fn on_keyboard_release<F>(&mut self, mut handler: F, prevent_default: bool)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_keyboard_release = Some(self.common.add_user_event(
            "keyup",
            move |event: KeyboardEvent| {
                if prevent_default {
                    event.prevent_default();
                }

                handler(
                    event::scan_code(&event),
                    event::virtual_key_code(&event),
                    event::keyboard_modifiers(&event),
                );
            },
        ));
    }

    pub fn on_keyboard_press<F>(&mut self, mut handler: F, prevent_default: bool)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_keyboard_press = Some(self.common.add_user_event(
            "keydown",
            move |event: KeyboardEvent| {
                // event.prevent_default() would suppress subsequent on_received_character() calls. That
                // suppression is correct for key sequences like Tab/Shift-Tab, Ctrl+R, PgUp/Down to
                // scroll, etc. We should not do it for key sequences that result in meaningful character
                // input though.
                if prevent_default {
                    let event_key = &event.key();
                    let is_key_string = event_key.len() == 1 || !event_key.is_ascii();
                    let is_shortcut_modifiers =
                        (event.ctrl_key() || event.alt_key()) && !event.get_modifier_state("AltGr");
                    if !is_key_string || is_shortcut_modifiers {
                        event.prevent_default();
                    }
                }

                handler(
                    event::scan_code(&event),
                    event::virtual_key_code(&event),
                    event::keyboard_modifiers(&event),
                );
            },
        ));
    }

    pub fn on_received_character<F>(&mut self, mut handler: F, prevent_default: bool)
    where
        F: 'static + FnMut(char),
    {
        // TODO: Use `beforeinput`.
        //
        // The `keypress` event is deprecated, but there does not seem to be a
        // viable/compatible alternative as of now. `beforeinput` is still widely
        // unsupported.
        self.on_received_character = Some(self.common.add_user_event(
            "keypress",
            move |event: KeyboardEvent| {
                // Suppress further handling to stop keys like the space key from scrolling the page.
                if prevent_default {
                    event.prevent_default();
                }

                handler(event::codepoint(&event));
            },
        ));
    }

    pub fn on_cursor_leave<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(i32),
    {
        match &mut self.mouse_state {
            MouseState::HasPointerEvent(h) => h.on_cursor_leave(&self.common, handler),
            MouseState::NoPointerEvent(h) => h.on_cursor_leave(&self.common, handler),
        }
    }

    pub fn on_cursor_enter<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(i32),
    {
        match &mut self.mouse_state {
            MouseState::HasPointerEvent(h) => h.on_cursor_enter(&self.common, handler),
            MouseState::NoPointerEvent(h) => h.on_cursor_enter(&self.common, handler),
        }
    }

    pub fn on_mouse_release<M, T>(&mut self, mouse_handler: M, touch_handler: T)
    where
        M: 'static + FnMut(i32, MouseButton, ModifiersState),
        T: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        match &mut self.mouse_state {
            MouseState::HasPointerEvent(h) => {
                h.on_mouse_release(&self.common, mouse_handler, touch_handler)
            }
            MouseState::NoPointerEvent(h) => h.on_mouse_release(&self.common, mouse_handler),
        }
    }

    pub fn on_mouse_press<M, T>(&mut self, mouse_handler: M, touch_handler: T)
    where
        M: 'static + FnMut(i32, PhysicalPosition<f64>, MouseButton, ModifiersState),
        T: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        match &mut self.mouse_state {
            MouseState::HasPointerEvent(h) => {
                h.on_mouse_press(&self.common, mouse_handler, touch_handler)
            }
            MouseState::NoPointerEvent(h) => h.on_mouse_press(&self.common, mouse_handler),
        }
    }

    pub fn on_cursor_move<M, T>(
        &mut self,
        mouse_handler: M,
        touch_handler: T,
        prevent_default: bool,
    ) where
        M: 'static + FnMut(i32, PhysicalPosition<f64>, PhysicalPosition<f64>, ModifiersState),
        T: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        match &mut self.mouse_state {
            MouseState::HasPointerEvent(h) => {
                h.on_cursor_move(&self.common, mouse_handler, touch_handler, prevent_default)
            }
            MouseState::NoPointerEvent(h) => h.on_cursor_move(&self.common, mouse_handler),
        }
    }

    pub fn on_touch_cancel<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        if let MouseState::HasPointerEvent(h) = &mut self.mouse_state {
            h.on_touch_cancel(&self.common, handler)
        }
    }

    pub fn on_mouse_wheel<F>(&mut self, mut handler: F, prevent_default: bool)
    where
        F: 'static + FnMut(i32, MouseScrollDelta, ModifiersState),
    {
        self.on_mouse_wheel = Some(self.common.add_event("wheel", move |event: WheelEvent| {
            if prevent_default {
                event.prevent_default();
            }

            if let Some(delta) = event::mouse_scroll_delta(&event) {
                handler(0, delta, event::mouse_modifiers(&event));
            }
        }));
    }

    pub fn on_fullscreen_change<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_fullscreen_change = Some(
            self.common
                .add_event("fullscreenchange", move |_: Event| handler()),
        );
    }

    pub fn on_dark_mode<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(bool),
    {
        let closure =
            Closure::wrap(
                Box::new(move |event: MediaQueryListEvent| handler(event.matches()))
                    as Box<dyn FnMut(_)>,
            );
        self.on_dark_mode = MediaQueryListHandle::new("(prefers-color-scheme: dark)", closure);
    }

    pub fn request_fullscreen(&self) {
        self.common.request_fullscreen()
    }

    pub fn is_fullscreen(&self) -> bool {
        self.common.is_fullscreen()
    }

    pub fn remove_listeners(&mut self) {
        self.on_focus = None;
        self.on_blur = None;
        self.on_keyboard_release = None;
        self.on_keyboard_press = None;
        self.on_received_character = None;
        self.on_mouse_wheel = None;
        self.on_fullscreen_change = None;
        self.on_dark_mode = None;
        match &mut self.mouse_state {
            MouseState::HasPointerEvent(h) => h.remove_listeners(),
            MouseState::NoPointerEvent(h) => h.remove_listeners(),
        }
    }
}

impl Common {
    fn add_event<E, F>(
        &self,
        event_name: &'static str,
        mut handler: F,
    ) -> EventListenerHandle<dyn FnMut(E)>
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

        EventListenerHandle::new(&self.raw, event_name, closure)
    }

    // The difference between add_event and add_user_event is that the latter has a special meaning
    // for browser security. A user event is a deliberate action by the user (like a mouse or key
    // press) and is the only time things like a fullscreen request may be successfully completed.)
    fn add_user_event<E, F>(
        &self,
        event_name: &'static str,
        mut handler: F,
    ) -> EventListenerHandle<dyn FnMut(E)>
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
        event_name: &'static str,
        mut handler: F,
    ) -> EventListenerHandle<dyn FnMut(MouseEvent)>
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

        let listener = EventListenerHandle::with_options(
            &window,
            event_name,
            closure,
            AddEventListenerOptions::new().capture(true),
        );

        listener
    }

    pub fn request_fullscreen(&self) {
        *self.wants_fullscreen.borrow_mut() = true;
    }

    pub fn is_fullscreen(&self) -> bool {
        super::is_fullscreen(&self.raw)
    }
}

/// Pointer events are supported or not.
enum MouseState {
    HasPointerEvent(pointer_handler::PointerHandler),
    NoPointerEvent(mouse_handler::MouseHandler),
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
