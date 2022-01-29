use super::event_handle::EventListenerHandle;
use crate::error::OsError as RootOE;
use crate::platform_impl::OsError;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{
    CompositionEvent, CssStyleDeclaration, HtmlInputElement, InputEvent, KeyboardEvent, MouseEvent,
};

const AGENT_ID: &str = "winit_input_agent";
pub struct Input {
    common: Common,
    on_composition_start: Option<EventListenerHandle<dyn FnMut(CompositionEvent)>>,
    on_composition_update: Option<EventListenerHandle<dyn FnMut(CompositionEvent)>>,
    on_composition_end: Option<EventListenerHandle<dyn FnMut(CompositionEvent)>>,
    on_focus_out: Option<EventListenerHandle<dyn FnMut(MouseEvent)>>,
    on_input: Option<EventListenerHandle<dyn FnMut(InputEvent)>>,
    on_key_down: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
}
struct Common {
    /// Note: resizing the HTMLCanvasElement should go through `backend::set_canvas_size` to ensure the DPI factor is maintained.
    raw: HtmlInputElement,
    composing: Rc<Cell<bool>>,
}
impl Input {
    pub fn create() -> Result<Self, RootOE> {
        let input: HtmlInputElement = {
            let window = web_sys::window()
                .ok_or(os_error!(OsError("Failed to obtain window".to_owned())))?;

            let document = window
                .document()
                .ok_or(os_error!(OsError("Failed to obtain document".to_owned())))?;

            document
                .create_element("input")
                .map_err(|_| os_error!(OsError("Failed to create input element".to_owned())))?
                .unchecked_into()
        };
        {
            let style = input.style();
            // Transparent
            style.set_property("opacity", "0").unwrap();
            // Hide under canvas
            style.set_property("z-index", "-1").unwrap();
        }
        input.set_id(AGENT_ID);
        input.set_size(1);
      //  input.set_hidden(true);
        input.set_autofocus(true);

        Ok(Self {
            common: Common {
                raw: input,
                composing: Rc::new(Cell::new(false)),
            },
            on_composition_start: None,
            on_composition_update: None,
            on_composition_end: None,
            on_focus_out: None,
            on_input: None,
            on_key_down: None,
        })
    }
    pub fn raw(&self) -> &HtmlInputElement {
        &self.common.raw
    }
    pub fn on_composition_start<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        let composing = self.common.composing.clone();
        let input = self.raw().clone();
        self.on_composition_start = Some(self.common.add_event(
            "compositionstart",
            move |_: CompositionEvent| {
                handler();
                composing.set(true);
                input.set_value("");
            },
        ));
    }
    pub fn on_composition_update<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(Option<String>),
    {
        self.on_composition_update = Some(self.common.add_event(
            "compositionupdate",
            move |event: CompositionEvent| {
                handler(event.data());
            },
        ));
    }

    pub fn on_composition_end<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(Option<String>),
    {
        let input = self.raw().clone();
        let composing = self.common.composing.clone();
        self.on_composition_end = Some(self.common.add_event(
            "compositionend",
            move |event: CompositionEvent| {
                handler(event.data());
                composing.set(false);
                event.stop_propagation();
                input.set_value("");
            },
        ));
    }

    pub fn on_input<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(Option<String>),
    {
        let input = self.raw().clone();
        let composing = self.common.composing.clone();
        self.on_input = Some(self.common.add_event("input", move |event: InputEvent| {
            if !composing.get() {
                handler(event.data());
                input.set_value("");
            }
        }));
    }
    pub fn on_keydown<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(KeyboardEvent),
    {
        let composing = self.common.composing.clone();
        self.on_key_down = Some(
            self.common
                .add_event("keydown", move |event: KeyboardEvent| {
                    if !composing.get() {
                        handler(event);
                    }
                }),
        );
    }
    pub fn style(&self) -> CssStyleDeclaration {
        self.common.raw.style()
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
                event_ref.stop_immediate_propagation();
                event_ref.cancel_bubble();
            }

            handler(event);
        }) as Box<dyn FnMut(E)>);

        let listener = EventListenerHandle::new(&self.raw, event_name, closure);

        listener
    }
}
