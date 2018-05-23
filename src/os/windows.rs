#![cfg(target_os = "windows")]

use std::os::raw::c_void;

use libc;
use winapi::shared::windef::HWND;

use {DeviceId, Icon, MonitorId, Window, WindowBuilder};

/// Additional methods on `Window` that are specific to Windows.
pub trait WindowExt {
    /// Returns the native handle that is used by this window.
    ///
    /// The pointer will become invalid when the native window was destroyed.
    fn get_hwnd(&self) -> *mut libc::c_void;

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>);
}

impl WindowExt for Window {
    #[inline]
    fn get_hwnd(&self) -> *mut libc::c_void {
        self.window.hwnd() as *mut _
    }

    #[inline]
    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>) {
        self.window.set_taskbar_icon(taskbar_icon)
    }
}

/// Additional methods on `WindowBuilder` that are specific to Windows.
pub trait WindowBuilderExt {
    /// Sets a parent to the window to be created.
    fn with_parent_window(self, parent: HWND) -> WindowBuilder;

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    fn with_taskbar_icon(self, taskbar_icon: Option<Icon>) -> WindowBuilder;

    /// When the window is crated, Windows emits a `WM_CREATE` message. This is the
    /// time where you can register application menus, context menus and
    /// custom window message IDs. Note that all the message IDs (ex. to track which
    /// context menu item was clicked) will come back to you in the form of
    /// `WindowEvent::Command(your_event_id)`.
    ///
    /// Due to multithreading-unsafety of the Win32 API, this can't be a regular
    /// message in the `EventsLoop`, since the `EventsLoop` is asynchronous and
    /// Windows will freeze the application if you try to add menus from another thread.
    /// The callback will be called immediately after the window has been created.
    fn with_create_callback(self, callback: fn(HWND) -> ()) -> WindowBuilder;
}

impl WindowBuilderExt for WindowBuilder {
    #[inline]
    fn with_parent_window(mut self, parent: HWND) -> WindowBuilder {
        self.platform_specific.parent = Some(parent);
        self
    }

    #[inline]
    fn with_taskbar_icon(mut self, taskbar_icon: Option<Icon>) -> WindowBuilder {
        self.platform_specific.taskbar_icon = taskbar_icon;
        self
    }

    #[inline]
    fn with_create_callback(mut self, callback: fn(HWND) -> ()) -> WindowBuilder {
        self.platform_specific.wm_create_callback = Some(callback);
        self
    }
}

/// Additional methods on `MonitorId` that are specific to Windows.
pub trait MonitorIdExt {
    /// Returns the name of the monitor adapter specific to the Win32 API.
    fn native_id(&self) -> String;

    /// Returns the handle of the monitor - `HMONITOR`.
    fn hmonitor(&self) -> *mut c_void;
}

impl MonitorIdExt for MonitorId {
    #[inline]
    fn native_id(&self) -> String {
        self.inner.get_native_identifier()
    }

    #[inline]
    fn hmonitor(&self) -> *mut c_void {
        self.inner.get_hmonitor() as *mut _
    }
}

/// Additional methods on `DeviceId` that are specific to Windows.
pub trait DeviceIdExt {
    /// Returns an identifier that persistently refers to this specific device.
    ///
    /// Will return `None` if the device is no longer available.
    fn get_persistent_identifier(&self) -> Option<String>;
}

impl DeviceIdExt for DeviceId {
    #[inline]
    fn get_persistent_identifier(&self) -> Option<String> {
        self.0.get_persistent_identifier()
    }
}
