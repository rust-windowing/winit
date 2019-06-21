#![cfg(any(target_os = "android"))]

use crate::{EventLoop, Window, WindowBuilder};
use std::os::raw::c_void;

/// Additional methods on `EventLoop` that are specific to Android.
pub trait EventLoopExtAndroid {
    /// Makes it possible for glutin to register a callback when a suspend event happens on Android
    fn set_suspend_callback(&self, cb: Option<Box<dyn Fn(bool) -> ()>>);
}

impl EventLoopExtAndroid for EventLoop {
    fn set_suspend_callback(&self, cb: Option<Box<dyn Fn(bool) -> ()>>) {
        self.event_loop.set_suspend_callback(cb);
    }
}

/// Additional methods on `Window` that are specific to Android.
pub trait WindowExtAndroid {
    fn native_window(&self) -> *const c_void;
}

impl WindowExtAndroid for Window {
    #[inline]
    fn native_window(&self) -> *const c_void {
        self.window.native_window()
    }
}

/// Additional methods on `WindowBuilder` that are specific to Android.
pub trait WindowBuilderExtAndroid {}

impl WindowBuilderExtAndroid for WindowBuilder {}
