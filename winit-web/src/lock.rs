use js_sys::{Object, Promise};
use once_cell::race::OnceBool;
use tracing::error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, DomException, Element, Navigator, console};

pub(crate) fn is_cursor_lock_raw(navigator: &Navigator, document: &Document) -> bool {
    static IS_CURSOR_LOCK_RAW: OnceBool = OnceBool::new();
    IS_CURSOR_LOCK_RAW.get_or_init(|| is_cursor_lock_raw_inner(navigator, document))
}

fn is_cursor_lock_raw_inner(navigator: &Navigator, document: &Document) -> bool {
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
        let reject_handler = Closure::new(|_| ());

        let promise: Promise = promise.unchecked_into();
        let _ = promise.catch(&reject_handler);
        true
    }
}

pub(crate) fn request_pointer_lock(navigator: &Navigator, document: &Document, element: &Element) {
    if is_cursor_lock_raw(navigator, document) {
        let reject_handler = Closure::new(|error: JsValue| {
            if let Some(error) = error.dyn_ref::<DomException>() {
                error!("Failed to lock pointer. {}: {}", error.name(), error.message());
            } else {
                console::error_1(&error);
                error!("Failed to lock pointer");
            }
        });

        let element: &ElementExt = element.unchecked_ref();
        let options: PointerLockOptions = Object::new().unchecked_into();
        options.set_unadjusted_movement(true);
        let _ = element.request_pointer_lock_with_options(&options).catch(&reject_handler);
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
