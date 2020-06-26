mod canvas;
mod event;
mod timeout;

pub use self::canvas::Canvas;
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

pub fn on_unload(mut handler: impl FnMut() + 'static) {
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
    use stdweb::*;

    let scale_factor = scale_factor();

    let physical_size = size.to_physical::<u32>(scale_factor);
    let logical_size = size.to_logical::<f64>(scale_factor);

    raw.set_width(physical_size.width);
    raw.set_height(physical_size.height);

    js! {
        @{raw.as_ref()}.style.width = @{logical_size.width} + "px";
        @{raw.as_ref()}.style.height = @{logical_size.height} + "px";
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
