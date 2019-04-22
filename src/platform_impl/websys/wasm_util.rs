extern crate wasm_bindgen;
extern crate web_sys;

use self::wasm_bindgen::prelude::*;

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