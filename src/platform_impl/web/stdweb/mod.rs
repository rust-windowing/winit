mod canvas;
mod event;
mod timeout;

pub use self::canvas::Canvas;
pub use self::timeout::Timeout;

use crate::platform::web::WindowExtStdweb;
use crate::window::Window;

use stdweb::js;
use stdweb::web::event::BeforeUnloadEvent;
use stdweb::web::html_element::CanvasElement;
use stdweb::web::window;
use stdweb::web::IEventTarget;

pub fn throw(msg: &str) {
    js! { throw @{msg} }
}

pub fn on_unload(mut handler: impl FnMut() + 'static) {
    window().add_event_listener(move |_: BeforeUnloadEvent| handler());
}

impl WindowExtStdweb for Window {
    fn canvas(&self) -> CanvasElement {
        self.window.canvas().raw().clone()
    }
}
