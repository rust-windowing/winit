mod canvas;
mod document;
mod event;
mod timeout;

pub use self::canvas::Canvas;
pub use self::document::Document;
pub use self::timeout::Timeout;

use crate::platform::web::WindowExtWebSys;
use crate::window::Window;
use web_sys::HtmlCanvasElement;

pub fn request_animation_frame<F>(f: F)
where
    F: Fn(),
{
}

pub fn throw(msg: &str) {
    wasm_bindgen::throw_str(msg);
}

impl WindowExtWebSys for Window {
    fn canvas(&self) -> HtmlCanvasElement {
        self.window.canvas().raw().clone()
    }
}
