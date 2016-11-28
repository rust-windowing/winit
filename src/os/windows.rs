#![cfg(target_os = "windows")]

use libc;
use Window;
use WindowBuilder;
use window;
use winapi;
use platform;

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
        self.window.platform_window()
    }
}

/// Additional methods on `WindowBuilder` that are specific to Windows.
pub trait WindowBuilderExt {
    fn with_parent_window(self, parent: window::WindowProxy) -> WindowBuilder;
}

impl WindowBuilderExt for WindowBuilder {
    /// Sets a parent to the window to be created
    #[inline]
    fn with_parent_window(mut self, parent: window::WindowProxy) -> WindowBuilder {
        self.platform_specific.parent = Some(parent);
        self
    }
}

impl WindowBuilderExt {
    /// Creates a new WindowProxy from a winapi::HWND
    #[inline]
    pub fn create_window_proxy_from_handle(handle: winapi::HWND) -> window::WindowProxy {
        window::WindowProxy::create_proxy(platform::WindowProxy{hwnd: handle})
    }
}
