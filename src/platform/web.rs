#![cfg(target_arch = "wasm32")]

//! The web target does not automatically insert the canvas element object into the web page, to
//! allow end users to determine how the page should be laid out. Use the `WindowExtStdweb` or
//! `WindowExtWebSys` traits (depending on your web backend) to retrieve the canvas from the
//! Window. Alternatively, use the `WindowBuilderExtStdweb` or `WindowBuilderExtWebSys` to provide
//! your own canvas.

use crate::window::WindowBuilder;

#[cfg(feature = "stdweb")]
use stdweb::web::html_element::CanvasElement;

#[cfg(feature = "stdweb")]
pub trait WindowExtStdweb {
    fn canvas(&self) -> CanvasElement;

    /// Whether the browser reports the preferred color scheme to be "dark".
    fn is_dark_mode(&self) -> bool;
}

#[cfg(feature = "web-sys")]
use web_sys::HtmlCanvasElement;

#[cfg(feature = "web-sys")]
pub trait WindowExtWebSys {
    fn canvas(&self) -> HtmlCanvasElement;

    /// Whether the browser reports the preferred color scheme to be "dark".
    fn is_dark_mode(&self) -> bool;
}

#[cfg(feature = "stdweb")]
pub trait WindowBuilderExtStdweb {
    fn with_canvas(self, canvas: Option<CanvasElement>) -> Self;
}

#[cfg(feature = "stdweb")]
impl WindowBuilderExtStdweb for WindowBuilder {
    fn with_canvas(mut self, canvas: Option<CanvasElement>) -> Self {
        self.platform_specific.canvas = canvas;

        self
    }
}

#[cfg(feature = "web-sys")]
pub trait WindowBuilderExtWebSys {
    fn with_canvas(self, canvas: Option<HtmlCanvasElement>) -> Self;
}

#[cfg(feature = "web-sys")]
impl WindowBuilderExtWebSys for WindowBuilder {
    fn with_canvas(mut self, canvas: Option<HtmlCanvasElement>) -> Self {
        self.platform_specific.canvas = canvas;

        self
    }
}
