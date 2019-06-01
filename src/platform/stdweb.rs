#![cfg(feature = "stdweb")]

use stdweb::web::html_element::CanvasElement;

pub trait WindowExtStdweb {
    fn canvas(&self) -> CanvasElement;
}

