#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

mod application;
mod coordinate_space;
mod device;
mod responder;
mod screen;
mod screen_mode;
mod view;
mod view_controller;
mod window;

pub(crate) use self::application::UIApplication;
pub(crate) use self::coordinate_space::UICoordinateSpace;
pub(crate) use self::device::UIDevice;
pub(crate) use self::responder::UIResponder;
pub(crate) use self::screen::{UIScreen, UIScreenOverscanCompensation};
pub(crate) use self::screen_mode::UIScreenMode;
#[allow(unused_imports)]
pub(crate) use self::view::{UIEdgeInsets, UIView};
pub(crate) use self::view_controller::{UIInterfaceOrientationMask, UIViewController};
pub(crate) use self::window::UIWindow;
