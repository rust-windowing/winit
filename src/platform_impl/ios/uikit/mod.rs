#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use std::os::raw::{c_char, c_int};

use objc2_foundation::NSString;

mod application;
mod coordinate_space;
mod device;
mod event;
mod geometry;
mod gesture_recognizer;
mod responder;
mod screen;
mod screen_mode;
mod status_bar_style;
mod touch;
mod trait_collection;
mod view;
mod view_controller;
mod window;

pub(crate) use self::application::UIApplication;
pub(crate) use self::coordinate_space::UICoordinateSpace;
pub(crate) use self::device::{UIDevice, UIUserInterfaceIdiom};
pub(crate) use self::event::UIEvent;
pub(crate) use self::geometry::UIRectEdge;
pub(crate) use self::gesture_recognizer::{
    UIGestureRecognizer, UIGestureRecognizerState, UIPinchGestureRecognizer,
    UIRotationGestureRecognizer, UITapGestureRecognizer,
};
pub(crate) use self::responder::UIResponder;
pub(crate) use self::screen::{UIScreen, UIScreenOverscanCompensation};
pub(crate) use self::screen_mode::UIScreenMode;
pub(crate) use self::status_bar_style::UIStatusBarStyle;
pub(crate) use self::touch::{UITouch, UITouchPhase, UITouchType};
pub(crate) use self::trait_collection::{UIForceTouchCapability, UITraitCollection};
#[allow(unused_imports)]
pub(crate) use self::view::{UIEdgeInsets, UIView};
pub(crate) use self::view_controller::{UIInterfaceOrientationMask, UIViewController};
pub(crate) use self::window::UIWindow;

#[link(name = "UIKit", kind = "framework")]
extern "C" {
    pub fn UIApplicationMain(
        argc: c_int,
        argv: *const c_char,
        principalClassName: Option<&NSString>,
        delegateClassName: Option<&NSString>,
    ) -> c_int;
}
