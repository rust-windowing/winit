use super::event;
use crate::dpi::{LogicalPosition, LogicalSize};
use crate::error::OsError as RootOE;
use crate::event::{ModifiersState, MouseButton, MouseScrollDelta, ScanCode, VirtualKeyCode};
use crate::platform_impl::OsError;

use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{FocusEvent, HtmlCanvasElement, KeyboardEvent, PointerEvent, WheelEvent};

pub struct Canvas {
    raw: HtmlCanvasElement,
    on_redraw: Closure<dyn Fn()>,
    on_focus: Option<Closure<dyn FnMut(FocusEvent)>>,
    on_blur: Option<Closure<dyn FnMut(FocusEvent)>>,
    on_key_up: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_key_down: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_mouse_out: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_over: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_up: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_down: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_move: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_scroll: Option<Closure<dyn FnMut(WheelEvent)>>,
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
            on_key_up: None,
            on_key_down: None,
            on_mouse_out: None,
            on_mouse_over: None,
            on_mouse_up: None,
            on_mouse_down: None,
            on_mouse_move: None,
            on_mouse_scroll: None,
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

    pub fn on_key_up<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_key_up = Some(self.add_event("keyup", move |event: KeyboardEvent| {
            handler(
                event::scan_code(&event),
                event::virtual_key_code(&event),
                event::keyboard_modifiers(&event),
            );
        }));
    }

    pub fn on_key_down<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_key_down = Some(self.add_event("keydown", move |event: KeyboardEvent| {
            handler(
                event::scan_code(&event),
                event::virtual_key_code(&event),
                event::keyboard_modifiers(&event),
            );
        }));
    }

    pub fn on_mouse_out<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        self.on_mouse_out = Some(self.add_event("pointerout", move |event: PointerEvent| {
            handler(event.pointer_id());
        }));
    }

    pub fn on_mouse_over<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        self.on_mouse_over = Some(self.add_event("pointerover", move |event: PointerEvent| {
            handler(event.pointer_id());
        }));
    }

    pub fn on_mouse_up<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton, ModifiersState),
    {
        self.on_mouse_up = Some(self.add_event("pointerup", move |event: PointerEvent| {
            handler(
                event.pointer_id(),
                event::mouse_button(&event),
                event::mouse_modifiers(&event),
            );
        }));
    }

    pub fn on_mouse_down<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton, ModifiersState),
    {
        let canvas = self.raw.clone();

        self.on_mouse_down = Some(self.add_event("pointerdown", move |event: PointerEvent| {
            // We focus the canvas manually when the user clicks on it.
            // This is necessary because we are preventing the default event behavior
            // in `add_event`
            canvas.focus().expect("Failed to focus canvas");

            handler(
                event.pointer_id(),
                event::mouse_button(&event),
                event::mouse_modifiers(&event),
            );
        }));
    }

    pub fn on_mouse_move<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, LogicalPosition, ModifiersState),
    {
        self.on_mouse_move = Some(self.add_event("pointermove", move |event: PointerEvent| {
            handler(
                event.pointer_id(),
                event::mouse_position(&event),
                event::mouse_modifiers(&event),
            );
        }));
    }

    pub fn on_mouse_scroll<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseScrollDelta, ModifiersState),
    {
        self.on_mouse_scroll = Some(self.add_event("wheel", move |event: WheelEvent| {
            if let Some(delta) = event::mouse_scroll_delta(&event) {
                handler(0, delta, event::mouse_modifiers(&event));
            }
        }));
    }

    fn add_event<E, F>(&self, event_name: &str, mut handler: F) -> Closure<FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        let closure = Closure::wrap(Box::new(move |event: E| {
            {
                let event_ref = event.as_ref();
                event_ref.prevent_default();
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
}
