mod canvas;
mod event;
mod timeout;

pub use self::canvas::Canvas;
pub use self::timeout::Timeout;

use crate::platform::web::WindowExtStdweb;
use crate::window::Window;

use stdweb::web::html_element::CanvasElement;

pub fn throw(msg: &str) {
    js! { throw @{msg} }
}

impl WindowExtStdweb for Window {
    fn canvas(&self) -> CanvasElement {
        self.window.canvas().raw().clone()
    }
}
