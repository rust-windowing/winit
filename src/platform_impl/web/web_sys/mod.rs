mod canvas;
mod event;
mod timeout;

pub use self::canvas::Canvas;
pub use self::timeout::Timeout;

use crate::platform::web::WindowExtWebSys;
use crate::window::Window;
use web_sys::HtmlCanvasElement;

pub fn throw(msg: &str) {
    wasm_bindgen::throw_str(msg);
}

pub fn exit_fullscreen() {
    let window = web_sys::window().expect("Failed to obtain window");
    let document = window.document().expect("Failed to obtain document");

    document.exit_fullscreen();
}

impl WindowExtWebSys for Window {
    fn canvas(&self) -> HtmlCanvasElement {
        self.window.canvas().raw().clone()
    }
}
