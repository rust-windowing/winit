use once_cell::unsync::OnceCell;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, HtmlCanvasElement};

thread_local! {
    static FULLSCREEN_API_SUPPORT: OnceCell<bool> = OnceCell::new();
}

fn canvas_has_fullscreen_api_support(canvas: &HtmlCanvasElement) -> bool {
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

fn document_has_fullscreen_api_support(document: &Document) -> bool {
    FULLSCREEN_API_SUPPORT.with(|support| {
        *support.get_or_init(|| {
            #[wasm_bindgen]
            extern "C" {
                type DocumentFullScreenApiSupport;

                #[wasm_bindgen(method, getter, js_name = exitFullscreen)]
                fn has_exit_fullscreen(this: &DocumentFullScreenApiSupport) -> JsValue;
            }

            let support: &DocumentFullScreenApiSupport = document.unchecked_ref();
            !support.has_exit_fullscreen().is_undefined()
        })
    })
}

pub fn request_fullscreen(canvas: &HtmlCanvasElement) -> Result<JsValue, JsValue> {
    #[wasm_bindgen]
    extern "C" {
        type RequestFullscreen;

        #[wasm_bindgen(catch, method, js_name = requestFullscreen)]
        fn request_fullscreen(this: &RequestFullscreen) -> Result<JsValue, JsValue>;

        #[wasm_bindgen(catch, method, js_name = webkitRequestFullscreen)]
        fn webkit_request_fullscreen(this: &RequestFullscreen) -> Result<JsValue, JsValue>;
    }

    let element: &RequestFullscreen = canvas.unchecked_ref();

    if canvas_has_fullscreen_api_support(canvas) {
        element.request_fullscreen()
    } else {
        element.webkit_request_fullscreen()
    }
}

pub fn exit_fullscreen(document: &Document) {
    #[wasm_bindgen]
    extern "C" {
        type ExitFullscreen;

        #[wasm_bindgen(method, js_name = webkitExitFullscreen)]
        fn webkit_exit_fullscreen(this: &ExitFullscreen);
    }

    if document_has_fullscreen_api_support(document) {
        #[allow(clippy::disallowed_methods)]
        document.exit_fullscreen()
    } else {
        let document: &ExitFullscreen = document.unchecked_ref();
        document.webkit_exit_fullscreen()
    }
}

pub fn fullscreen_element(document: &Document) -> Option<Element> {
    #[wasm_bindgen]
    extern "C" {
        type FullscreenElement;

        #[wasm_bindgen(method, getter, js_name = webkitFullscreenElement)]
        fn webkit_fullscreen_element(this: &FullscreenElement) -> Option<Element>;
    }

    if document_has_fullscreen_api_support(document) {
        #[allow(clippy::disallowed_methods)]
        document.fullscreen_element()
    } else {
        let document: &FullscreenElement = document.unchecked_ref();
        document.webkit_fullscreen_element()
    }
}
