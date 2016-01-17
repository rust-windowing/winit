#![cfg(target_os = "macos")]

use std::os::raw::c_void;
use Window;

/// Additional methods on `Window` that are specific to MacOS.
pub trait WindowExt {
    /// Returns a pointer to the cocoa `NSWindow` that is used by this window.
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_nswindow(&self) -> *mut c_void;
}

impl WindowExt for Window {
    #[inline]
    fn get_nswindow(&self) -> *mut c_void {
        self.window.platform_window() as *mut c_void
    }
}
