#![deprecated(since = "0.23.0", note = "Please migrate to web-sys over stdweb")]

mod canvas;
mod event;
mod scaling;
mod timeout;

pub use self::canvas::Canvas;
pub use self::scaling::ScaleChangeDetector;
pub use self::timeout::{AnimationFrameRequest, Timeout};

use crate::dpi::{LogicalSize, Size};
use crate::platform::web::WindowExtStdweb;
use crate::window::Window;

use stdweb::js;
use stdweb::unstable::TryInto;
use stdweb::web::event::BeforeUnloadEvent;
use stdweb::web::window;
use stdweb::web::IEventTarget;
use stdweb::web::{document, html_element::CanvasElement, Element};

pub fn throw(msg: &str) {
    js! { throw @{msg} }
}

pub fn exit_fullscreen() {
    document().exit_fullscreen();
}

pub type UnloadEventHandle = ();

pub fn on_unload(mut handler: impl FnMut() + 'static) -> UnloadEventHandle {
    window().add_event_listener(move |_: BeforeUnloadEvent| handler());
}

impl WindowExtStdweb for Window {
    fn canvas(&self) -> CanvasElement {
        self.window.canvas().raw().clone()
    }

    fn is_dark_mode(&self) -> bool {
        // TODO: upstream to stdweb
        let is_dark_mode = js! {
            return (window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches)
        };

        is_dark_mode.try_into().expect("should return a bool")
    }
}

pub fn window_size() -> LogicalSize<f64> {
    let window = window();
    let width = window.inner_width() as f64;
    let height = window.inner_height() as f64;

    LogicalSize { width, height }
}

pub fn scale_factor() -> f64 {
    let window = window();
    window.device_pixel_ratio()
}

pub fn set_canvas_size(raw: &CanvasElement, size: Size) {
    let scale_factor = scale_factor();

    let physical_size = size.to_physical::<u32>(scale_factor);
    let logical_size = size.to_logical::<f64>(scale_factor);

    raw.set_width(physical_size.width);
    raw.set_height(physical_size.height);

    set_canvas_style_property(raw, "width", &format!("{}px", logical_size.width));
    set_canvas_style_property(raw, "height", &format!("{}px", logical_size.height));
}

pub fn set_canvas_style_property(raw: &CanvasElement, style_attribute: &str, value: &str) {
    js! {
        @{raw.as_ref()}.style[@{style_attribute}] = @{value};
    }
}

pub fn is_fullscreen(canvas: &CanvasElement) -> bool {
    match document().fullscreen_element() {
        Some(elem) => {
            let raw: Element = canvas.clone().into();
            raw == elem
        }
        None => false,
    }
}

pub type RawCanvasType = CanvasElement;
