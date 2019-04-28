extern crate wasm_bindgen;
extern crate web_sys;

use platform_impl::Window as CanvasWindow;
use platform_impl::window::ElementSelection;
use window::{Window, WindowBuilder};
use platform::websys::wasm_bindgen::prelude::*;
use platform::websys::wasm_bindgen::JsCast;

pub trait WebsysWindowExt {
    fn get_canvas<'a>(&'a self) -> &'a web_sys::HtmlCanvasElement;
}

impl WebsysWindowExt for Window {
    fn get_canvas<'a>(&'a self) -> &'a web_sys::HtmlCanvasElement {
        &self.window.canvas
    }
}

pub trait WebsysWindowBuilderExt {
    fn with_canvas_id(mut self, canvas_id: &str) -> WindowBuilder;

    fn with_container_id(mut self, container_id: &str) -> WindowBuilder;
}

impl WebsysWindowBuilderExt for WindowBuilder {
    fn with_canvas_id(mut self, canvas_id: &str) -> WindowBuilder {
        self.platform_specific.element = ElementSelection::CanvasId(canvas_id.to_string());
        self
    }

    fn with_container_id(mut self, container_id: &str) -> WindowBuilder {
        self.platform_specific.element = ElementSelection::ContainerId(container_id.to_string());
        self
    }
}
