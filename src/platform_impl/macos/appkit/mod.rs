#![deny(unsafe_op_in_unsafe_fn)]
// Objective-C methods have different conventions, and it's much easier to
// understand if we just use the same names
#![allow(non_snake_case)]

mod application;
mod cursor;
mod image;
mod responder;
mod view;
mod window;

pub(crate) use self::application::NSApplication;
pub(crate) use self::cursor::NSCursor;
pub(crate) use self::image::NSImage;
pub(crate) use self::responder::NSResponder;
pub(crate) use self::view::NSView;
pub(crate) use self::window::NSWindow;
