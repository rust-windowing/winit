//! iOS support
//!
//! # Building app
//! To build ios app you will need rustc built for this targets:
//!
//!  - armv7-apple-ios
//!  - armv7s-apple-ios
//!  - i386-apple-ios
//!  - aarch64-apple-ios
//!  - x86_64-apple-ios
//!
//! Then
//!
//! ```
//! cargo build --target=...
//! ```
//! The simplest way to integrate your app into xcode environment is to build it
//! as a static library. Wrap your main function and export it.
//!
//! ```rust, ignore
//! #[no_mangle]
//! pub extern fn start_winit_app() {
//!     start_inner()
//! }
//!
//! fn start_inner() {
//!    ...
//! }
//! ```
//!
//! Compile project and then drag resulting .a into Xcode project. Add winit.h to xcode.
//!
//! ```ignore
//! void start_winit_app();
//! ```
//!
//! Use start_winit_app inside your xcode's main function.
//!
//!
//! # App lifecycle and events
//!
//! iOS environment is very different from other platforms and you must be very
//! careful with it's events. Familiarize yourself with
//! [app lifecycle](https://developer.apple.com/library/ios/documentation/UIKit/Reference/UIApplicationDelegate_Protocol/).
//!
//!
//! This is how those event are represented in winit:
//!
//!  - applicationDidBecomeActive is Resumed
//!  - applicationWillResignActive is Suspended
//!  - applicationWillTerminate is LoopDestroyed
//!
//! Keep in mind that after LoopDestroyed event is received every attempt to draw with
//! opengl will result in segfault.
//!
//! Also note that app may not receive the LoopDestroyed event if suspended; it might be SIGKILL'ed.

#![cfg(ios_platform)]
#![allow(clippy::let_unit_value)]

// TODO: (mtak-) UIKit requires main thread for virtually all function/method calls. This could be
// worked around in the future by using GCD (grand central dispatch) and/or caching of values like
// window size/position.
macro_rules! assert_main_thread {
    ($($t:tt)*) => {
        if !::objc2::foundation::is_main_thread() {
            panic!($($t)*);
        }
    };
}

mod app_state;
mod event_loop;
mod ffi;
mod monitor;
mod uikit;
mod view;
mod window;

use std::fmt;

pub(crate) use self::{
    event_loop::{
        EventLoop, EventLoopProxy, EventLoopWindowTarget, PlatformSpecificEventLoopAttributes,
    },
    monitor::{MonitorHandle, VideoMode},
    window::{PlatformSpecificWindowBuilderAttributes, Window, WindowId},
};

use self::uikit::UIScreen;
pub(crate) use crate::icon::NoIcon as PlatformIcon;
pub(self) use crate::platform_impl::Fullscreen;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId {
    uiscreen: *const UIScreen,
}

impl DeviceId {
    pub const unsafe fn dummy() -> Self {
        DeviceId {
            uiscreen: std::ptr::null(),
        }
    }
}

unsafe impl Send for DeviceId {}
unsafe impl Sync for DeviceId {}

#[derive(Debug)]
pub enum OsError {}

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "os error")
    }
}
