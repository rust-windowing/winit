#![cfg(any(target_os = "android"))]

use std::os::raw::c_void;
use Window;
use WindowBuilder;

/// Additional methods on `Window` that are specific to Android.
pub trait WindowExt {
    fn get_native_window(&self) -> *const c_void;
}

impl WindowExt for Window {
    #[inline]
    fn get_native_window(&self) -> *const c_void {
        self.window.get_native_window()
    }
}

/// Additional methods on `WindowBuilder` that are specific to Android.
pub trait WindowBuilderExt {

}

impl WindowBuilderExt for WindowBuilder {
}
