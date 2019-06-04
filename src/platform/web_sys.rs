#![cfg(feature = "web-sys")]

use web_sys::HtmlCanvasElement;

pub trait WindowExtWebSys {
    fn canvas(&self) -> HtmlCanvasElement;
}
