#![cfg(any(target_os = "android"))]

use crate::event_loop::{EventLoop, EventLoopWindowTarget};
use crate::window::{Window, WindowBuilder};

/// Additional methods on `EventLoop` that are specific to Android.
pub trait EventLoopExtAndroid {}

impl<T> EventLoopExtAndroid for EventLoop<T> {}

/// Additional methods on `EventLoopWindowTarget` that are specific to Android.
pub trait EventLoopWindowTargetExtAndroid {
    /// Makes it possible for to register a callback when a suspend event
    /// happens on Android.
    fn set_suspend_callback(&self, cb: Option<Box<dyn Fn(bool) -> ()>>);
}

impl<T> EventLoopWindowTargetExtAndroid for EventLoopWindowTarget<T> {
    fn set_suspend_callback(&self, cb: Option<Box<dyn Fn(bool) -> ()>>) {
        self.p.set_suspend_callback(cb);
    }
}

/// Additional methods on `Window` that are specific to Android.
pub trait WindowExtAndroid {}

impl WindowExtAndroid for Window {}

/// Additional methods on `WindowBuilder` that are specific to Android.
pub trait WindowBuilderExtAndroid {}

impl WindowBuilderExtAndroid for WindowBuilder {}
