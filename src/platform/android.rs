use crate::{
    event_loop::{EventLoop, EventLoopWindowTarget},
    window::{Window, WindowBuilder},
};
use ndk::configuration::Configuration;
use ndk_glue::Rect;

/// Additional methods on [`EventLoop`] that are specific to Android.
pub trait EventLoopExtAndroid {}

impl<T> EventLoopExtAndroid for EventLoop<T> {}

/// Additional methods on [`EventLoopWindowTarget`] that are specific to Android.
pub trait EventLoopWindowTargetExtAndroid {}

/// Additional methods on [`Window`] that are specific to Android.
pub trait WindowExtAndroid {
    fn content_rect(&self) -> Rect;

    fn config(&self) -> Configuration;
}

impl WindowExtAndroid for Window {
    fn content_rect(&self) -> Rect {
        self.window.content_rect()
    }

    fn config(&self) -> Configuration {
        self.window.config()
    }
}

impl<T> EventLoopWindowTargetExtAndroid for EventLoopWindowTarget<T> {}

/// Additional methods on [`WindowBuilder`] that are specific to Android.
pub trait WindowBuilderExtAndroid {}

impl WindowBuilderExtAndroid for WindowBuilder {}
