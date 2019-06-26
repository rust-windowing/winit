#[cfg(feature = "use_stdweb")]
use stdweb::web::html_element::CanvasElement;

#[cfg(feature = "use_stdweb")]
pub trait WindowExtStdweb {
    fn canvas(&self) -> CanvasElement;
}

#[cfg(feature = "use_web-sys")]
use web_sys::HtmlCanvasElement;

#[cfg(feature = "use_web-sys")]
pub trait WindowExtWebSys {
    fn canvas(&self) -> HtmlCanvasElement;
}
