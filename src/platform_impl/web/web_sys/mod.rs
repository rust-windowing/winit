mod canvas;
mod event;
mod timeout;

pub use self::canvas::Canvas;
pub use self::timeout::Timeout;

use crate::platform::web::WindowExtWebSys;
use crate::window::Window;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{BeforeUnloadEvent, HtmlCanvasElement};

pub fn throw(msg: &str) {
    wasm_bindgen::throw_str(msg);
}

pub fn on_unload(mut handler: impl FnMut() + 'static) {
    let window = web_sys::window().expect("Failed to obtain window");

    let closure = Closure::wrap(
        Box::new(move |_: BeforeUnloadEvent| handler()) as Box<dyn FnMut(BeforeUnloadEvent)>
    );

    window
        .add_event_listener_with_callback("beforeunload", &closure.as_ref().unchecked_ref())
        .expect("Failed to add close listener");
}

impl WindowExtWebSys for Window {
    fn canvas(&self) -> HtmlCanvasElement {
        self.window.canvas().raw().clone()
    }
}
