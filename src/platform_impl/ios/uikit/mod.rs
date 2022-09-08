#![deny(unsafe_op_in_unsafe_fn)]

mod responder;
mod view;
mod view_controller;
mod window;

pub(crate) use self::responder::UIResponder;
#[allow(unused)]
pub(crate) use self::view::UIView;
#[allow(unused)]
pub(crate) use self::view_controller::UIViewController;
#[allow(unused)]
pub(crate) use self::window::UIWindow;
