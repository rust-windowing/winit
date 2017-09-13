#![cfg(target_os = "windows")]

use libc;
use MonitorId;
use Window;
use WindowBuilder;
use winapi;

/// Additional methods on `Window` that are specific to Windows.
pub trait WindowExt {
    /// Returns the native handle that is used by this window.
    ///
    /// The pointer will become invalid when the native window was destroyed.
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
    /// Sets a parent to the window to be created.
    #[inline]
    fn with_parent_window(mut self, parent: winapi::HWND) -> WindowBuilder {
        self.platform_specific.parent = Some(parent);
        self
    }
}

/// Additional methods on `MonitorId` that are specific to Windows.
pub trait MonitorIdExt {
    /// Returns the name of the monitor specific to the Win32 API.
    fn native_id(&self) -> String;
}

impl MonitorIdExt for MonitorId {
    #[inline]
    fn native_id(&self) -> String {
        self.inner.get_native_identifier()
    }
}
