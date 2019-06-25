use crate::dpi::LogicalSize;
use crate::error::OsError as RootOE;
use crate::platform_impl::OsError;

use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

pub struct Canvas {
    raw: HtmlCanvasElement,
}

impl Canvas {
    pub fn create() -> Result<Self, RootOE> {
        let window = web_sys::window().expect("Failed to obtain window");
        let document = window.document().expect("Failed to obtain document");

        let canvas: HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?
            .unchecked_into();

        document
            .body()
            .ok_or_else(|| os_error!(OsError("Failed to find body node".to_owned())))?
            .append_child(&canvas)
            .map_err(|_| os_error!(OsError("Failed to append canvas".to_owned())))?;

        Ok(Canvas { raw: canvas })
    }

    pub fn set_attribute(&self, attribute: &str, value: &str) {
        self.raw
            .set_attribute(attribute, value)
            .expect(&format!("Set attribute: {}", attribute));
    }

    pub fn position(&self) -> (f64, f64) {
        let bounds = self.raw.get_bounding_client_rect();

        (bounds.x(), bounds.y())
    }

    pub fn width(&self) -> f64 {
        self.raw.width() as f64
    }

    pub fn height(&self) -> f64 {
        self.raw.height() as f64
    }

    pub fn set_size(&self, size: LogicalSize) {
        self.raw.set_width(size.width as u32);
        self.raw.set_height(size.height as u32);
    }

    pub fn raw(&self) -> HtmlCanvasElement {
        self.raw.clone()
    }

    pub fn on_mouse_out<F>(&self, f: F) {}
    pub fn on_mouse_over<F>(&self, f: F) {}
    pub fn on_mouse_up<F>(&self, f: F) {}
    pub fn on_mouse_down<F>(&self, f: F) {}
    pub fn on_mouse_move<F>(&self, f: F) {}
    pub fn on_mouse_scroll<F>(&self, f: F) {}
}
