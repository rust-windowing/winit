use super::event_handle::EventListenerHandle;
use super::media_query_handle::MediaQueryListHandle;
use super::pointer::PointerHandler;
use super::{event, ButtonsState};
use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::error::OsError as RootOE;
use crate::event::{Force, MouseButton, MouseScrollDelta};
use crate::keyboard::{Key, KeyCode, KeyLocation, ModifiersState};
use crate::platform_impl::{OsError, PlatformSpecificWindowBuilderAttributes};

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Promise;
use smol_str::SmolStr;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Event, FocusEvent, HtmlCanvasElement, KeyboardEvent, MediaQueryListEvent, WheelEvent,
};

#[allow(dead_code)]
pub struct Canvas {
    common: Common,
    on_touch_start: Option<EventListenerHandle<dyn FnMut(Event)>>,
    on_touch_end: Option<EventListenerHandle<dyn FnMut(Event)>>,
    on_focus: Option<EventListenerHandle<dyn FnMut(FocusEvent)>>,
    on_blur: Option<EventListenerHandle<dyn FnMut(FocusEvent)>>,
    on_keyboard_release: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
    on_keyboard_press: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
    on_mouse_wheel: Option<EventListenerHandle<dyn FnMut(WheelEvent)>>,
    on_fullscreen_change: Option<EventListenerHandle<dyn FnMut(Event)>>,
    on_dark_mode: Option<MediaQueryListHandle>,
    pointer_handler: PointerHandler,
}

pub struct Common {
    /// Note: resizing the HTMLCanvasElement should go through `backend::set_canvas_size` to ensure the DPI factor is maintained.
    pub raw: HtmlCanvasElement,
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
            on_mouse_wheel: None,
            on_fullscreen_change: None,
            on_dark_mode: None,
            pointer_handler: PointerHandler::new(),
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
        F: 'static + FnMut(KeyCode, Key, Option<SmolStr>, KeyLocation, bool, ModifiersState),
    {
        self.on_keyboard_release = Some(self.common.add_user_event(
            "keyup",
            move |event: KeyboardEvent| {
                if prevent_default {
                    event.prevent_default();
                }
                let key = event::key(&event);
                let modifiers = event::keyboard_modifiers(&event);
                handler(
                    event::key_code(&event),
                    key,
                    event::key_text(&event),
                    event::key_location(&event),
                    event.repeat(),
                    modifiers,
                );
            },
        ));
    }

    pub fn on_keyboard_press<F>(&mut self, mut handler: F, prevent_default: bool)
    where
        F: 'static + FnMut(KeyCode, Key, Option<SmolStr>, KeyLocation, bool, ModifiersState),
    {
        self.on_keyboard_press = Some(self.common.add_user_event(
            "keydown",
            move |event: KeyboardEvent| {
                if prevent_default {
                    event.prevent_default();
                }
                let key = event::key(&event);
                let modifiers = event::keyboard_modifiers(&event);
                handler(
                    event::key_code(&event),
                    key,
                    event::key_text(&event),
                    event::key_location(&event),
                    event.repeat(),
                    modifiers,
                );
            },
        ));
    }

    pub fn on_cursor_leave<MOD, M>(&mut self, modifier_handler: MOD, mouse_handler: M)
    where
        MOD: 'static + FnMut(ModifiersState),
        M: 'static + FnMut(i32),
    {
        self.pointer_handler
            .on_cursor_leave(&self.common, modifier_handler, mouse_handler)
    }

    pub fn on_cursor_enter<MOD, M>(&mut self, modifier_handler: MOD, mouse_handler: M)
    where
        MOD: 'static + FnMut(ModifiersState),
        M: 'static + FnMut(i32),
    {
        self.pointer_handler
            .on_cursor_enter(&self.common, modifier_handler, mouse_handler)
    }

    pub fn on_mouse_release<MOD, M, T>(
        &mut self,
        modifier_handler: MOD,
        mouse_handler: M,
        touch_handler: T,
    ) where
        MOD: 'static + FnMut(ModifiersState),
        M: 'static + FnMut(i32, PhysicalPosition<f64>, MouseButton),
        T: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        self.pointer_handler.on_mouse_release(
            &self.common,
            modifier_handler,
            mouse_handler,
            touch_handler,
        )
    }

    pub fn on_mouse_press<MOD, M, T>(
        &mut self,
        modifier_handler: MOD,
        mouse_handler: M,
        touch_handler: T,
        prevent_default: bool,
    ) where
        MOD: 'static + FnMut(ModifiersState),
        M: 'static + FnMut(i32, PhysicalPosition<f64>, MouseButton),
        T: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        self.pointer_handler.on_mouse_press(
            &self.common,
            modifier_handler,
            mouse_handler,
            touch_handler,
            prevent_default,
        )
    }

    pub fn on_cursor_move<MOD, M, T, B>(
        &mut self,
        modifier_handler: MOD,
        mouse_handler: M,
        touch_handler: T,
        button_handler: B,
        prevent_default: bool,
    ) where
        MOD: 'static + FnMut(ModifiersState),
        M: 'static + FnMut(i32, PhysicalPosition<f64>, PhysicalPosition<f64>),
        T: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
        B: 'static + FnMut(i32, PhysicalPosition<f64>, ButtonsState, MouseButton),
    {
        self.pointer_handler.on_cursor_move(
            &self.common,
            modifier_handler,
            mouse_handler,
            touch_handler,
            button_handler,
            prevent_default,
        )
    }

    pub fn on_touch_cancel<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        self.pointer_handler.on_touch_cancel(&self.common, handler)
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
                let modifiers = event::mouse_modifiers(&event);
                handler(0, delta, modifiers);
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
        self.on_mouse_wheel = None;
        self.on_fullscreen_change = None;
        self.on_dark_mode = None;
        self.pointer_handler.remove_listeners()
    }
}

impl Common {
    pub fn add_event<E, F>(
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
    pub fn add_user_event<E, F>(
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

    pub fn request_fullscreen(&self) {
        #[wasm_bindgen]
        extern "C" {
            type ElementExt;

            #[wasm_bindgen(catch, method, js_name = requestFullscreen)]
            fn request_fullscreen(this: &ElementExt) -> Result<JsValue, JsValue>;
        }

        let raw: &ElementExt = self.raw.unchecked_ref();

        // This should return a `Promise`, but Safari v<16.4 is not up-to-date with the spec.
        match raw.request_fullscreen() {
            Ok(value) if !value.is_undefined() => {
                let promise: Promise = value.unchecked_into();
                let wants_fullscreen = self.wants_fullscreen.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if JsFuture::from(promise).await.is_err() {
                        *wants_fullscreen.borrow_mut() = true
                    }
                });
            }
            // We are on Safari v<16.4, let's try again on the next transient activation.
            _ => *self.wants_fullscreen.borrow_mut() = true,
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        super::is_fullscreen(&self.raw)
    }
}
