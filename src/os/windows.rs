#![cfg(target_os = "windows")]

use libc;
use Window;
use WindowBuilder;
use winapi;

/// Additional methods on `Window` that are specific to Windows.
pub trait WindowExt {
    /// Returns a pointer to the `Window` object of xlib that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_hwnd(&self) -> *mut libc::c_void;
}

impl WindowExt for Window {
    #[inline]
    fn get_hwnd(&self) -> *mut libc::c_void {
        self.window.hwnd() as *mut _
    }
}

/// Additional methods on `WindowBuilder` that are specific to Windows.
pub trait WindowBuilderExt {
    fn with_parent_window(self, parent: winapi::HWND) -> WindowBuilder;
}

impl WindowBuilderExt for WindowBuilder {
    /// Sets a parent to the window to be created
    #[inline]
    fn with_parent_window(mut self, parent: winapi::HWND) -> WindowBuilder {
        self.platform_specific.parent = Some(parent);
        self
    }
}
