use std::cell::Cell;
use std::rc::Rc;

use js_sys::Promise;
use once_cell::unsync::OnceCell;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, HtmlCanvasElement};

use super::EventListenerHandle;

thread_local! {
    static FULLSCREEN_API_SUPPORT: OnceCell<bool> = const { OnceCell::new() };
}

pub struct FullscreenHandler {
    document: Document,
    canvas: HtmlCanvasElement,
    fullscreen_requested: Rc<Cell<bool>>,
    _fullscreen_change: EventListenerHandle<dyn FnMut()>,
}

impl FullscreenHandler {
    pub fn new(document: Document, canvas: HtmlCanvasElement) -> Self {
        let fullscreen_requested = Rc::new(Cell::new(false));
        let fullscreen_change = EventListenerHandle::new(
            canvas.clone(),
            if has_fullscreen_api_support(&canvas) {
                "fullscreenchange"
            } else {
                "webkitfullscreenchange"
            },
            Closure::new({
                let fullscreen_requested = fullscreen_requested.clone();
                move || {
                    // It doesn't matter if the canvas entered or exitted fullscreen mode,
                    // we don't want to request it again later.
                    fullscreen_requested.set(false);
                }
            }),
        );

        Self {
            document,
            canvas,
            fullscreen_requested,
            _fullscreen_change: fullscreen_change,
        }
    }

    fn internal_request_fullscreen(&self) {
        #[wasm_bindgen]
        extern "C" {
            type RequestFullscreen;

            #[wasm_bindgen(method, js_name = requestFullscreen)]
            fn request_fullscreen(this: &RequestFullscreen) -> Promise;

            #[wasm_bindgen(method, js_name = webkitRequestFullscreen)]
            fn webkit_request_fullscreen(this: &RequestFullscreen);
        }

        let canvas: &RequestFullscreen = self.canvas.unchecked_ref();

        if has_fullscreen_api_support(&self.canvas) {
            thread_local! {
                static REJECT_HANDLER: Closure<dyn FnMut(JsValue)> = Closure::new(|_| ());
            }
            REJECT_HANDLER.with(|handler| {
                let _ = canvas.request_fullscreen().catch(handler);
            });
        } else {
            canvas.webkit_request_fullscreen();
        }
    }

    pub fn request_fullscreen(&self) {
        if !self.is_fullscreen() {
            self.internal_request_fullscreen();
            self.fullscreen_requested.set(true);
        }
    }

    pub fn transient_activation(&self) {
        if self.fullscreen_requested.get() {
            self.internal_request_fullscreen()
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        #[wasm_bindgen]
        extern "C" {
            type FullscreenElement;

            #[wasm_bindgen(method, getter, js_name = webkitFullscreenElement)]
            fn webkit_fullscreen_element(this: &FullscreenElement) -> Option<Element>;
        }

        let element = if has_fullscreen_api_support(&self.canvas) {
            #[allow(clippy::disallowed_methods)]
            self.document.fullscreen_element()
        } else {
            let document: &FullscreenElement = self.document.unchecked_ref();
            document.webkit_fullscreen_element()
        };

        match element {
            Some(element) => {
                let canvas: &Element = &self.canvas;
                canvas == &element
            }
            None => false,
        }
    }

    pub fn exit_fullscreen(&self) {
        #[wasm_bindgen]
        extern "C" {
            type ExitFullscreen;

            #[wasm_bindgen(method, js_name = webkitExitFullscreen)]
            fn webkit_exit_fullscreen(this: &ExitFullscreen);
        }

        if has_fullscreen_api_support(&self.canvas) {
            #[allow(clippy::disallowed_methods)]
            self.document.exit_fullscreen()
        } else {
            let document: &ExitFullscreen = self.document.unchecked_ref();
            document.webkit_exit_fullscreen()
        }

        self.fullscreen_requested.set(false);
    }

    pub fn cancel(&self) {
        self.fullscreen_requested.set(false);
    }
}

fn has_fullscreen_api_support(canvas: &HtmlCanvasElement) -> bool {
    FULLSCREEN_API_SUPPORT.with(|support| {
        *support.get_or_init(|| {
            #[wasm_bindgen]
            extern "C" {
                type CanvasFullScreenApiSupport;

                #[wasm_bindgen(method, getter, js_name = requestFullscreen)]
                fn has_request_fullscreen(this: &CanvasFullScreenApiSupport) -> JsValue;
            }

            let support: &CanvasFullScreenApiSupport = canvas.unchecked_ref();
            !support.has_request_fullscreen().is_undefined()
        })
    })
}
