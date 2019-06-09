use wasm_bindgen::prelude::*;
use ::error::OsError as WOsError;

use super::OsError;

// A macro to provide `println!(..)`-style syntax for `console.log` logging.
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen(inline_js = "export function js_exit() { throw 'hacky exit!'; }")]
extern "C" {
    pub fn js_exit();
}
 
impl From<wasm_bindgen::JsValue> for WOsError {
    fn from(error: wasm_bindgen::JsValue) -> Self {
        os_error!(OsError{})
    }
}
