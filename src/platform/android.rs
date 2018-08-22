#![cfg(any(target_os = "android"))]

use std::os::raw::c_void;
use EventLoop;
use Window;
use WindowBuilder;

/// Additional methods on `EventLoop` that are specific to Android.
pub trait EventLoopExtAndroid {
    /// Makes it possible for glutin to register a callback when a suspend event happens on Android
    fn set_suspend_callback(&self, cb: Option<Box<Fn(bool) -> ()>>);
}

impl EventLoopExtAndroid for EventLoop {
    fn set_suspend_callback(&self, cb: Option<Box<Fn(bool) -> ()>>) {
        self.events_loop.set_suspend_callback(cb);
    }
}

/// Additional methods on `Window` that are specific to Android.
pub trait WindowExtAndroid {
    fn get_native_window(&self) -> *const c_void;
}

impl WindowExtAndroid for Window {
    #[inline]
    fn get_native_window(&self) -> *const c_void {
        self.window.get_native_window()
    }
}

/// Additional methods on `WindowBuilder` that are specific to Android.
pub trait WindowBuilderExtAndroid {

}

impl WindowBuilderExtAndroid for WindowBuilder {
}
