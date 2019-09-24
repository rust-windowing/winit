#![cfg(target_arch = "wasm32")]

//! The web target does not automatically insert the canvas element object into the web page, to
//! allow end users to determine how the page should be laid out. Use the `WindowExtStdweb` or
//! `WindowExtWebSys` traits (depending on your web backend) to retrieve the canvas from the
//! Window.

#[cfg(feature = "stdweb")]
use stdweb::web::html_element::CanvasElement;

#[cfg(feature = "stdweb")]
pub trait WindowExtStdweb {
    fn canvas(&self) -> CanvasElement;
}

#[cfg(feature = "web-sys")]
use web_sys::HtmlCanvasElement;

#[cfg(feature = "web-sys")]
pub trait WindowExtWebSys {
    fn canvas(&self) -> HtmlCanvasElement;
}
