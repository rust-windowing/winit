mod canvas;
mod event;
mod timeout;

pub use self::canvas::Canvas;
pub use self::timeout::Timeout;

use crate::dpi::LogicalSize;
use crate::platform::web::WindowExtWebSys;
use crate::window::Window;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{window, BeforeUnloadEvent, Element, HtmlCanvasElement};

pub fn throw(msg: &str) {
    wasm_bindgen::throw_str(msg);
}

pub fn exit_fullscreen() {
    let window = web_sys::window().expect("Failed to obtain window");
    let document = window.document().expect("Failed to obtain document");

    document.exit_fullscreen();
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

pub fn window_size() -> LogicalSize {
    let window = web_sys::window().expect("Failed to obtain window");
    let width = window
        .inner_width()
        .expect("Failed to get width")
        .as_f64()
        .expect("Failed to get width as f64");
    let height = window
        .inner_height()
        .expect("Failed to get height")
        .as_f64()
        .expect("Failed to get height as f64");

    LogicalSize { width, height }
}

pub fn is_fullscreen(canvas: &HtmlCanvasElement) -> bool {
    let window = window().expect("Failed to obtain window");
    let document = window.document().expect("Failed to obtain document");

    match document.fullscreen_element() {
        Some(elem) => {
            let raw: Element = canvas.clone().into();
            raw == elem
        }
        None => false,
    }
}
