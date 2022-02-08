use super::event;
use super::event_handle::EventListenerHandle;
use crate::error::OsError as RootOE;
use crate::platform_impl::OsError;
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{CompositionEvent, CssStyleDeclaration, HtmlInputElement, KeyboardEvent};

pub struct Input {
    common: Common,
    on_composition_start: Option<EventListenerHandle<dyn FnMut(CompositionEvent)>>,
    on_composition_update: Option<EventListenerHandle<dyn FnMut(CompositionEvent)>>,
    on_composition_end: Option<EventListenerHandle<dyn FnMut(CompositionEvent)>>,
    on_input: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
    on_key_down: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
}
struct Common {
    raw: HtmlInputElement,
    end: Rc<Cell<bool>>,
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

        input.set_size(1);
        //  input.set_hidden(true);
        input.set_autofocus(true);

        Ok(Self {
            common: Common {
                raw: input,
                end: Rc::new(Cell::new(false)),
                composing: Rc::new(Cell::new(false)),
            },
            on_composition_start: None,
            on_composition_update: None,
            on_composition_end: None,
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
        let input = self.raw().clone();
        let composing = self.common.composing.clone();
        self.on_composition_start = Some(self.common.add_event(
            "compositionstart",
            move |event: CompositionEvent| {
                //do not emit it many times.
                if !composing.get() {
                    web_sys::console::log_1(&event);
                    handler();
                    input.set_value("");
                }
                composing.set(true);
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
                web_sys::console::log_1(&event);
                handler(event.data());
            },
        ));
    }

    pub fn on_composition_end<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(Option<String>),
    {
        let input = self.raw().clone();
        let end = self.common.end.clone();
        let composing = self.common.composing.clone();
        self.on_composition_end = Some(self.common.add_event(
            "compositionend",
            move |event: CompositionEvent| {
                web_sys::console::log_1(&event);
                handler(event.data());
                input.set_value("");
                end.set(true);
                composing.set(false);
            },
        ));
    }

    pub fn on_input<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(char),
    {
        let input = self.raw().clone();
        let end = self.common.end.clone();
        let composing = self.common.composing.clone();
        self.on_input = Some(
            self.common
                .add_event("keypress", move |event: KeyboardEvent| {
                    web_sys::console::log_1(&event);

                    if !end.get() & !composing.get() {
                        input.set_value("");
                        handler(event::codepoint(&event));
                    }
                    if !event.is_composing() {
                        end.set(false);
                    }
                }),
        );
    }

    pub fn on_keydown<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(KeyboardEvent),
    {
        self.on_key_down = Some(
            self.common
                .add_event("keydown", move |event: KeyboardEvent| {
                    web_sys::console::log_1(&event);
                    handler(event)
                }),
        );
    }

    pub fn style(&self) -> CssStyleDeclaration {
        self.common.raw.style()
    }
    pub fn remove_listeners(&mut self) {
        self.on_key_down = None;
        self.on_input = None;
        self.on_composition_start = None;
        self.on_composition_update = None;
        self.on_composition_end = None;
    }
    pub fn set_attribute(&self, attribute: &str, value: &str) {
        self.common
            .raw
            .set_attribute(attribute, value)
            .expect(&format!("Set attribute: {}", attribute));
    }
}
impl Common {
    fn add_event<E, F>(
        &self,
        event_name: &'static str,
        handler: F,
    ) -> EventListenerHandle<dyn FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        let closure = Closure::wrap(Box::new(handler) as Box<dyn FnMut(E)>);

        let listener = EventListenerHandle::new(&self.raw, event_name, closure);

        listener
    }
}
