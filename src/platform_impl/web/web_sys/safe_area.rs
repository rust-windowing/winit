use dpi::{LogicalPosition, LogicalSize};
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlHtmlElement, Window};

use super::Style;

pub struct SafeAreaHandle {
    style: Style,
}

impl SafeAreaHandle {
    pub fn new(window: &Window, document: &Document) -> Self {
        let document: HtmlHtmlElement = document.document_element().unwrap().unchecked_into();
        #[allow(clippy::disallowed_methods)]
        let write = document.style();
        write
            .set_property(
                "--__winit_safe_area",
                "env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) \
                 env(safe-area-inset-left)",
            )
            .expect("unexpected read-only declaration block");
        #[allow(clippy::disallowed_methods)]
        let read = window
            .get_computed_style(&document)
            .expect("failed to obtain computed style")
            // this can't fail: we aren't using a pseudo-element
            .expect("invalid pseudo-element");

        SafeAreaHandle { style: Style { read, write } }
    }

    pub fn get(&self) -> (LogicalPosition<f64>, LogicalSize<f64>) {
        let value = self.style.get("--__winit_safe_area");

        let mut values = value
            .split(' ')
            .map(|value| value.strip_suffix("px").expect("unexpected unit other then `px` found"));
        let top: f64 = values.next().unwrap().parse().unwrap();
        let right: f64 = values.next().unwrap().parse().unwrap();
        let bottom: f64 = values.next().unwrap().parse().unwrap();
        let left: f64 = values.next().unwrap().parse().unwrap();
        assert_eq!(values.next(), None, "unexpected fifth value");

        let width = super::style_size_property(&self.style, "width") - left - right;
        let height = super::style_size_property(&self.style, "height") - top - bottom;

        (LogicalPosition::new(left, top), LogicalSize::new(width, height))
    }
}

impl Drop for SafeAreaHandle {
    fn drop(&mut self) {
        self.style.remove("--__winit_safe_area");
    }
}
