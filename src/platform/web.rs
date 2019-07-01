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
