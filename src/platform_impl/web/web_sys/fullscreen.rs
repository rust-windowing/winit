use std::cell::OnceCell;

use js_sys::{Object, Promise};
use tracing::error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{console, Document, Element, HtmlCanvasElement, Window};

use super::super::main_thread::MainThreadMarker;
use super::super::monitor::{self, ScreenDetailed};
use crate::platform_impl::Fullscreen;

pub(crate) fn request_fullscreen(
    main_thread: MainThreadMarker,
    window: &Window,
    document: &Document,
    canvas: &HtmlCanvasElement,
    fullscreen: Fullscreen,
) {
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(extends = HtmlCanvasElement)]
        type RequestFullscreen;

        #[wasm_bindgen(method, js_name = requestFullscreen)]
        fn request_fullscreen(this: &RequestFullscreen) -> Promise;

        #[wasm_bindgen(method, js_name = requestFullscreen)]
        fn request_fullscreen_with_options(
            this: &RequestFullscreen,
            options: &FullscreenOptions,
        ) -> Promise;

        #[wasm_bindgen(method, js_name = webkitRequestFullscreen)]
        fn webkit_request_fullscreen(this: &RequestFullscreen);

        type FullscreenOptions;

        #[wasm_bindgen(method, setter, js_name = screen)]
        fn set_screen(this: &FullscreenOptions, screen: &ScreenDetailed);
    }

    thread_local! {
        static REJECT_HANDLER: Closure<dyn FnMut(JsValue)> = Closure::new(|error| {
            console::error_1(&error);
            error!("Failed to transition to full screen mode")
        });
    }

    if is_fullscreen(document, canvas) {
        return;
    }

    let canvas: &RequestFullscreen = canvas.unchecked_ref();

    match fullscreen {
        Fullscreen::Exclusive(_) => error!("Exclusive full screen mode is not supported"),
        Fullscreen::Borderless(Some(monitor)) => {
            if !monitor::has_screen_details_support(window) {
                error!(
                    "Fullscreen mode selecting a specific screen is not supported by this browser"
                );
                return;
            }

            if let Some(monitor) = monitor.detailed(main_thread) {
                let options: FullscreenOptions = Object::new().unchecked_into();
                options.set_screen(&monitor);
                REJECT_HANDLER.with(|handler| {
                    let _ = canvas.request_fullscreen_with_options(&options).catch(handler);
                });
            } else {
                error!(
                    "Selecting a specific screen for fullscreen mode requires a detailed screen. \
                     See `MonitorHandleExtWeb::is_detailed()`."
                )
            }
        },
        Fullscreen::Borderless(None) => {
            if has_fullscreen_api_support(canvas) {
                REJECT_HANDLER.with(|handler| {
                    let _ = canvas.request_fullscreen().catch(handler);
                });
            } else {
                canvas.webkit_request_fullscreen();
            }
        },
    }
}

pub fn is_fullscreen(document: &Document, canvas: &HtmlCanvasElement) -> bool {
    #[wasm_bindgen]
    extern "C" {
        type FullscreenElement;

        #[wasm_bindgen(method, getter, js_name = webkitFullscreenElement)]
        fn webkit_fullscreen_element(this: &FullscreenElement) -> Option<Element>;
    }

    let element = if has_fullscreen_api_support(canvas) {
        #[allow(clippy::disallowed_methods)]
        document.fullscreen_element()
    } else {
        let document: &FullscreenElement = document.unchecked_ref();
        document.webkit_fullscreen_element()
    };

    match element {
        Some(element) => {
            let canvas: &Element = canvas;
            canvas == &element
        },
        None => false,
    }
}

pub fn exit_fullscreen(document: &Document, canvas: &HtmlCanvasElement) {
    #[wasm_bindgen]
    extern "C" {
        type ExitFullscreen;

        #[wasm_bindgen(method, js_name = webkitExitFullscreen)]
        fn webkit_exit_fullscreen(this: &ExitFullscreen);
    }

    if has_fullscreen_api_support(canvas) {
        #[allow(clippy::disallowed_methods)]
        document.exit_fullscreen()
    } else {
        let document: &ExitFullscreen = document.unchecked_ref();
        document.webkit_exit_fullscreen()
    }
}

fn has_fullscreen_api_support(canvas: &HtmlCanvasElement) -> bool {
    thread_local! {
        static FULLSCREEN_API_SUPPORT: OnceCell<bool> = const { OnceCell::new() };
    }

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
