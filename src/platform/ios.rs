#![cfg(target_os = "ios")]

use std::os::raw::c_void;

use crate::event_loop::EventLoop;
use crate::monitor::MonitorHandle;
use crate::window::{Window, WindowBuilder};

/// Additional methods on `EventLoop` that are specific to iOS.
pub trait EventLoopExtIOS {
    /// Returns the idiom (phone/tablet/tv/etc) for the current device.
    fn idiom(&self) -> Idiom;
}

impl<T: 'static> EventLoopExtIOS for EventLoop<T> {
    fn idiom(&self) -> Idiom {
        self.event_loop.idiom()
    }
}

/// Additional methods on `Window` that are specific to iOS.
pub trait WindowExtIOS {
    /// Returns a pointer to the `UIWindow` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn ui_window(&self) -> *mut c_void;

    /// Returns a pointer to the `UIViewController` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn ui_view_controller(&self) -> *mut c_void;

    /// Returns a pointer to the `UIView` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn ui_view(&self) -> *mut c_void;

    /// Sets the HiDpi factor used by this window.
    ///
    /// This translates to `-[UIWindow setContentScaleFactor:hidpi_factor]`.
    fn set_hidpi_factor(&self, hidpi_factor: f64);

    /// Sets the valid orientations for screens showing this `Window`.
    ///
    /// On iPhones and iPods upside down portrait is never enabled.
    fn set_valid_orientations(&self, valid_orientations: ValidOrientations);
}

impl WindowExtIOS for Window {
    #[inline]
    fn ui_window(&self) -> *mut c_void {
        self.window.ui_window() as _
    }

    #[inline]
    fn ui_view_controller(&self) -> *mut c_void {
        self.window.ui_view_controller() as _
    }

    #[inline]
    fn ui_view(&self) -> *mut c_void {
        self.window.ui_view() as _
    }

    #[inline]
    fn set_hidpi_factor(&self, hidpi_factor: f64) {
        self.window.set_hidpi_factor(hidpi_factor)
    }

    #[inline]
    fn set_valid_orientations(&self, valid_orientations: ValidOrientations) {
        self.window.set_valid_orientations(valid_orientations)
    }
}

/// Additional methods on `WindowBuilder` that are specific to iOS.
pub trait WindowBuilderExtIOS {
    /// Sets the root view class used by the `Window`, otherwise a barebones `UIView` is provided.
    ///
    /// The class will be initialized by calling `[root_view initWithFrame:CGRect]`
    fn with_root_view_class(self, root_view_class: *const c_void) -> WindowBuilder;

    /// Sets the `contentScaleFactor` of the underlying `UIWindow` to `hidpi_factor`.
    ///
    /// The default value is device dependent, and it's recommended GLES or Metal applications set
    /// this to `MonitorHandle::hidpi_factor()`.
    fn with_hidpi_factor(self, hidpi_factor: f64) -> WindowBuilder;

    /// Sets the valid orientations for the `Window`.
    fn with_valid_orientations(self, valid_orientations: ValidOrientations) -> WindowBuilder;
}

impl WindowBuilderExtIOS for WindowBuilder {
    #[inline]
    fn with_root_view_class(mut self, root_view_class: *const c_void) -> WindowBuilder {
        self.platform_specific.root_view_class = unsafe { &*(root_view_class as *const _) };
        self
    }

    #[inline]
    fn with_hidpi_factor(mut self, hidpi_factor: f64) -> WindowBuilder {
        self.platform_specific.hidpi_factor = Some(hidpi_factor);
        self
    }

    #[inline]
    fn with_valid_orientations(mut self, valid_orientations: ValidOrientations) -> WindowBuilder {
        self.platform_specific.valid_orientations = valid_orientations;
        self
    }
}

/// Additional methods on `MonitorHandle` that are specific to iOS.
pub trait MonitorHandleExtIOS {
    /// Returns a pointer to the `UIScreen` that is used by this monitor.
    fn ui_screen(&self) -> *mut c_void;
}

impl MonitorHandleExtIOS for MonitorHandle {
    #[inline]
    fn ui_screen(&self) -> *mut c_void {
        self.inner.ui_screen() as _
    }
}

/// Valid orientations for a particular `Window`.
#[derive(Clone, Copy, Debug)]
pub enum ValidOrientations {
    /// Excludes `PortraitUpsideDown` on iphone
    LandscapeAndPortrait,

    Landscape,

    /// Excludes `PortraitUpsideDown` on iphone
    Portrait,
}

impl Default for ValidOrientations {
    #[inline]
    fn default() -> ValidOrientations {
        ValidOrientations::LandscapeAndPortrait
    }
}

/// The device [idiom].
///
/// [idiom]: https://developer.apple.com/documentation/uikit/uidevice/1620037-userinterfaceidiom?language=objc
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Idiom {
    Unspecified,

    /// iPhone and iPod touch.
    Phone,

    /// iPad.
    Pad,

    /// tvOS and Apple TV.
    TV,
    CarPlay,
}
