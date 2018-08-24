#![cfg(target_os = "ios")]

use std::os::raw::c_void;

use {MonitorHandle, Window, WindowBuilder};

/// Additional methods on `Window` that are specific to iOS.
pub trait WindowExtIOS {
    /// Returns a pointer to the `UIWindow` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn get_uiwindow(&self) -> *mut c_void;

    /// Returns a pointer to the `UIView` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn get_uiview(&self) -> *mut c_void;
}

impl WindowExtIOS for Window {
    #[inline]
    fn get_uiwindow(&self) -> *mut c_void {
        self.window.get_uiwindow() as _
    }

    #[inline]
    fn get_uiview(&self) -> *mut c_void {
        self.window.get_uiview() as _
    }
}

/// Additional methods on `WindowBuilder` that are specific to iOS.
pub trait WindowBuilderExtIOS {
    /// Sets the root view class used by the `Window`, otherwise a barebones `UIView` is provided.
    ///
    /// The class will be initialized by calling `[root_view initWithFrame:CGRect]`
    fn with_root_view_class(self, root_view_class: *const c_void) -> WindowBuilder;
}

impl WindowBuilderExtIOS for WindowBuilder {
    #[inline]
    fn with_root_view_class(mut self, root_view_class: *const c_void) -> WindowBuilder {
        self.platform_specific.root_view_class = unsafe { &*(root_view_class as *const _) };
        self
    }
}

/// Additional methods on `MonitorHandle` that are specific to iOS.
pub trait MonitorHandleExtIOS {
    /// Returns a pointer to the `UIScreen` that is used by this monitor.
    fn get_uiscreen(&self) -> *mut c_void;
}

impl MonitorHandleExtIOS for MonitorHandle {
    #[inline]
    fn get_uiscreen(&self) -> *mut c_void {
        self.inner.get_uiscreen() as _
    }
}
