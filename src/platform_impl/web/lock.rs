use std::cell::OnceCell;

use js_sys::{Object, Promise};
use tracing::error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{console, Document, DomException, Element, Navigator};

pub(crate) fn is_cursor_lock_raw(navigator: &Navigator, document: &Document) -> bool {
    thread_local! {
        static IS_CURSOR_LOCK_RAW: OnceCell<bool> = const { OnceCell::new() };
    }

    IS_CURSOR_LOCK_RAW.with(|cell| {
        *cell.get_or_init(|| {
            // TODO: Remove when Chrome can better advertise that they don't support unaccelerated
            // movement on Linux.
            // See <https://issues.chromium.org/issues/40833850>.
            if super::web_sys::chrome_linux(navigator) {
                return false;
            }

            let element: ElementExt = document.create_element("div").unwrap().unchecked_into();
            let promise = element.request_pointer_lock();

            if promise.is_undefined() {
                false
            } else {
                thread_local! {
                    static REJECT_HANDLER: Closure<dyn FnMut(JsValue)> = Closure::new(|_| ());
                }

                let promise: Promise = promise.unchecked_into();
                let _ = REJECT_HANDLER.with(|handler| promise.catch(handler));
                true
            }
        })
    })
}

pub(crate) fn request_pointer_lock(navigator: &Navigator, document: &Document, element: &Element) {
    if is_cursor_lock_raw(navigator, document) {
        thread_local! {
            static REJECT_HANDLER: Closure<dyn FnMut(JsValue)> = Closure::new(|error: JsValue| {
                if let Some(error) = error.dyn_ref::<DomException>() {
                        error!("Failed to lock pointer. {}: {}", error.name(), error.message());
                } else {
                    console::error_1(&error);
                    error!("Failed to lock pointer");
                }
            });
        }

        let element: &ElementExt = element.unchecked_ref();
        let options: PointerLockOptions = Object::new().unchecked_into();
        options.set_unadjusted_movement(true);
        let _ = REJECT_HANDLER
            .with(|handler| element.request_pointer_lock_with_options(&options).catch(handler));
    } else {
        element.request_pointer_lock();
    }
}

#[wasm_bindgen]
extern "C" {
    type ElementExt;

    #[wasm_bindgen(method, js_name = requestPointerLock)]
    fn request_pointer_lock(this: &ElementExt) -> JsValue;

    #[wasm_bindgen(method, js_name = requestPointerLock)]
    fn request_pointer_lock_with_options(
        this: &ElementExt,
        options: &PointerLockOptions,
    ) -> Promise;

    type PointerLockOptions;

    #[wasm_bindgen(method, setter, js_name = unadjustedMovement)]
    fn set_unadjusted_movement(this: &PointerLockOptions, value: bool);
}
