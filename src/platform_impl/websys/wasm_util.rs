use wasm_bindgen::prelude::*;
use window::CreationError;

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
 
impl From<wasm_bindgen::JsValue> for CreationError {
    fn from(error: wasm_bindgen::JsValue) -> Self {
        CreationError::OsError(error.as_string().unwrap_or("Window error".to_string()))
    }
}