mod canvas;
mod event;
mod event_handle;
mod media_query_handle;
mod scaling;
mod timeout;

pub use self::canvas::Canvas;
pub use self::scaling::ScaleChangeDetector;
pub use self::timeout::{AnimationFrameRequest, Timeout};

use crate::dpi::{LogicalSize, Size};
use crate::platform::web::WindowExtWebSys;
use crate::window::Window;
use wasm_bindgen::closure::Closure;
use web_sys::{window, BeforeUnloadEvent, Element, HtmlCanvasElement};

pub fn throw(msg: &str) {
    wasm_bindgen::throw_str(msg);
}

pub fn exit_fullscreen() {
    let window = web_sys::window().expect("Failed to obtain window");
    let document = window.document().expect("Failed to obtain document");

    document.exit_fullscreen();
}

pub struct UnloadEventHandle {
    _listener: event_handle::EventListenerHandle<dyn FnMut(BeforeUnloadEvent)>,
}

pub fn on_unload(mut handler: impl FnMut() + 'static) -> UnloadEventHandle {
    let window = web_sys::window().expect("Failed to obtain window");

    let closure = Closure::wrap(
        Box::new(move |_: BeforeUnloadEvent| handler()) as Box<dyn FnMut(BeforeUnloadEvent)>
    );

    let listener = event_handle::EventListenerHandle::new(&window, "beforeunload", closure);
    UnloadEventHandle {
        _listener: listener,
    }
}

impl WindowExtWebSys for Window {
    fn canvas(&self) -> HtmlCanvasElement {
        self.window.canvas().raw().clone()
    }

    fn is_dark_mode(&self) -> bool {
        let window = web_sys::window().expect("Failed to obtain window");

        window
            .match_media("(prefers-color-scheme: dark)")
            .ok()
            .flatten()
            .map(|media| media.matches())
            .unwrap_or(false)
    }
}

pub fn window_size() -> LogicalSize<f64> {
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

pub fn scale_factor() -> f64 {
    let window = web_sys::window().expect("Failed to obtain window");
    window.device_pixel_ratio()
}

pub fn set_canvas_size(raw: &HtmlCanvasElement, size: Size) {
    let scale_factor = scale_factor();

    let physical_size = size.to_physical::<u32>(scale_factor);
    let logical_size = size.to_logical::<f64>(scale_factor);

    raw.set_width(physical_size.width);
    raw.set_height(physical_size.height);

    set_canvas_style_property(raw, "width", &format!("{}px", logical_size.width));
    set_canvas_style_property(raw, "height", &format!("{}px", logical_size.height));
}

pub fn set_canvas_style_property(raw: &HtmlCanvasElement, property: &str, value: &str) {
    let style = raw.style();
    style
        .set_property(property, value)
        .unwrap_or_else(|err| panic!("error: {err:?}\nFailed to set {property}"))
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

pub type RawCanvasType = HtmlCanvasElement;
