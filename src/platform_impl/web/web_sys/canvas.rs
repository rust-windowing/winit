use super::event;
use crate::dpi::{LogicalPosition, LogicalSize};
use crate::error::OsError as RootOE;
use crate::event::{ModifiersState, MouseButton, MouseScrollDelta, ScanCode, VirtualKeyCode};
use crate::platform_impl::OsError;

use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{Element, FocusEvent, HtmlCanvasElement, KeyboardEvent, PointerEvent, WheelEvent};

pub struct Canvas {
    raw: HtmlCanvasElement,
    on_redraw: Closure<dyn Fn()>,
    on_focus: Option<Closure<dyn FnMut(FocusEvent)>>,
    on_blur: Option<Closure<dyn FnMut(FocusEvent)>>,
    on_keyboard_release: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_keyboard_press: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_received_character: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_cursor_leave: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_cursor_enter: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_cursor_move: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_press: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_release: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_wheel: Option<Closure<dyn FnMut(WheelEvent)>>,
}

impl Drop for Canvas {
    fn drop(&mut self) {
        self.raw.remove();
    }
}

impl Canvas {
    pub fn create<F>(on_redraw: F) -> Result<Self, RootOE>
    where
        F: 'static + Fn(),
    {
        let window = web_sys::window().expect("Failed to obtain window");
        let document = window.document().expect("Failed to obtain document");

        let canvas: HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?
            .unchecked_into();

        document
            .body()
            .ok_or_else(|| os_error!(OsError("Failed to find body node".to_owned())))?
            .append_child(&canvas)
            .map_err(|_| os_error!(OsError("Failed to append canvas".to_owned())))?;

        // TODO: Set up unique ids
        canvas
            .set_attribute("tabindex", "0")
            .expect("Failed to set a tabindex");

        Ok(Canvas {
            raw: canvas,
            on_redraw: Closure::wrap(Box::new(on_redraw) as Box<dyn Fn()>),
            on_blur: None,
            on_focus: None,
            on_keyboard_release: None,
            on_keyboard_press: None,
            on_received_character: None,
            on_cursor_leave: None,
            on_cursor_enter: None,
            on_cursor_move: None,
            on_mouse_release: None,
            on_mouse_press: None,
            on_mouse_wheel: None,
        })
    }

    pub fn set_attribute(&self, attribute: &str, value: &str) {
        self.raw
            .set_attribute(attribute, value)
            .expect(&format!("Set attribute: {}", attribute));
    }

    pub fn position(&self) -> (f64, f64) {
        let bounds = self.raw.get_bounding_client_rect();

        (bounds.x(), bounds.y())
    }

    pub fn width(&self) -> f64 {
        self.raw.width() as f64
    }

    pub fn height(&self) -> f64 {
        self.raw.height() as f64
    }

    pub fn set_size(&self, size: LogicalSize) {
        self.raw.set_width(size.width as u32);
        self.raw.set_height(size.height as u32);
    }

    pub fn raw(&self) -> &HtmlCanvasElement {
        &self.raw
    }

    pub fn request_redraw(&self) {
        let window = web_sys::window().expect("Failed to obtain window");
        window
            .request_animation_frame(&self.on_redraw.as_ref().unchecked_ref())
            .expect("Failed to request animation frame");
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
        self.on_keyboard_release = Some(self.add_event("keyup", move |event: KeyboardEvent| {
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
        self.on_keyboard_press = Some(self.add_event("keydown", move |event: KeyboardEvent| {
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
        self.on_received_character =
            Some(self.add_event("keypress", move |event: KeyboardEvent| {
                handler(event::codepoint(&event));
            }));
    }

    pub fn on_cursor_leave<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        self.on_cursor_leave = Some(self.add_event("pointerout", move |event: PointerEvent| {
            handler(event.pointer_id());
        }));
    }

    pub fn on_cursor_enter<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        self.on_cursor_enter = Some(self.add_event("pointerover", move |event: PointerEvent| {
            handler(event.pointer_id());
        }));
    }

    pub fn on_mouse_release<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton, ModifiersState),
    {
        self.on_mouse_release = Some(self.add_event("pointerup", move |event: PointerEvent| {
            handler(
                event.pointer_id(),
                event::mouse_button(&event),
                event::mouse_modifiers(&event),
            );
        }));
    }

    pub fn on_mouse_press<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton, ModifiersState),
    {
        self.on_mouse_press = Some(self.add_event("pointerdown", move |event: PointerEvent| {
            handler(
                event.pointer_id(),
                event::mouse_button(&event),
                event::mouse_modifiers(&event),
            );
        }));
    }

    pub fn on_cursor_move<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, LogicalPosition, ModifiersState),
    {
        self.on_cursor_move = Some(self.add_event("pointermove", move |event: PointerEvent| {
            handler(
                event.pointer_id(),
                event::mouse_position(&event),
                event::mouse_modifiers(&event),
            );
        }));
    }

    pub fn on_mouse_wheel<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseScrollDelta, ModifiersState),
    {
        self.on_mouse_wheel = Some(self.add_event("wheel", move |event: WheelEvent| {
            if let Some(delta) = event::mouse_scroll_delta(&event) {
                handler(0, delta, event::mouse_modifiers(&event));
            }
        }));
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

    pub fn request_fullscreen(&self) {
        self.raw.request_fullscreen().expect("Fullscreen failed");
    }

    pub fn is_fullscreen(&self) -> bool {
        let window = web_sys::window().expect("Failed to obtain window");
        let document = window.document().expect("Failed to obtain document");

        match document.fullscreen_element() {
            Some(elem) => {
                let raw: Element = self.raw.clone().into();
                raw == elem
            }
            None => false,
        }
    }
}
