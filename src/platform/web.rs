#![cfg(target_arch = "wasm32")]

//! The web target does not automatically insert the canvas element object into the web page, to
//! allow end users to determine how the page should be laid out. Use the `WindowExtWebSys` trait
//! to retrieve the canvas from the Window. Alternatively, use the `WindowBuilderExtWebSys` trait
//! to provide your own canvas.

use crate::window::WindowBuilder;

use web_sys::HtmlCanvasElement;

pub trait WindowExtWebSys {
    fn canvas(&self) -> HtmlCanvasElement;

    /// Whether the browser reports the preferred color scheme to be "dark".
    fn is_dark_mode(&self) -> bool;
}

pub trait WindowBuilderExtWebSys {
    fn with_canvas(self, canvas: Option<HtmlCanvasElement>) -> Self;
}

impl WindowBuilderExtWebSys for WindowBuilder {
    fn with_canvas(mut self, canvas: Option<HtmlCanvasElement>) -> Self {
        self.platform_specific.canvas = canvas;

        self
    }
}
