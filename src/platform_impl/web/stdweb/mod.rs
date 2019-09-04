mod canvas;
mod event;
mod timeout;

pub use self::canvas::Canvas;
pub use self::timeout::Timeout;

use crate::platform::web::WindowExtStdweb;
use crate::window::Window;

use stdweb::web::{document, html_element::CanvasElement};

pub fn throw(msg: &str) {
    js! { throw @{msg} }
}

pub fn exit_fullscreen() {
    document().exit_fullscreen();
}

impl WindowExtStdweb for Window {
    fn canvas(&self) -> CanvasElement {
        self.window.canvas().raw().clone()
    }
}
