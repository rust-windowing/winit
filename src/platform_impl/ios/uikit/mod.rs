#![deny(unsafe_op_in_unsafe_fn)]

mod responder;
mod view;
mod view_controller;
mod window;

pub(crate) use self::responder::UIResponder;
#[allow(unused)]
pub(crate) use self::view::UIView;
pub(crate) use self::view_controller::UIViewController;
pub(crate) use self::window::UIWindow;
