#![cfg(target_os = "windows")]

use std::os::raw::c_void;

use libc;
use winapi::shared::windef::HWND;

use event::DeviceId;
use monitor::MonitorHandle;
use event_loop::EventLoop;
use window::{Icon, Window, WindowBuilder};
use platform_impl::EventLoop as WindowsEventLoop;

/// Additional methods on `EventLoop` that are specific to Windows.
pub trait EventLoopExtWindows {
    /// By default, winit on Windows will attempt to enable process-wide DPI awareness. If that's
    /// undesirable, you can create an `EventLoop` using this function instead.
    fn new_dpi_unaware() -> Self where Self: Sized;
}

impl<T> EventLoopExtWindows for EventLoop<T> {
    #[inline]
    fn new_dpi_unaware() -> Self {
        EventLoop {
            event_loop: WindowsEventLoop::with_dpi_awareness(false),
            _marker: ::std::marker::PhantomData,
        }
    }
}

/// Additional methods on `Window` that are specific to Windows.
pub trait WindowExtWindows {
    /// Returns the native handle that is used by this window.
    ///
    /// The pointer will become invalid when the native window was destroyed.
    fn get_hwnd(&self) -> *mut libc::c_void;

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>);
}

impl WindowExtWindows for Window {
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
pub trait WindowBuilderExtWindows {
    /// Sets a parent to the window to be created.
    fn with_parent_window(self, parent: HWND) -> WindowBuilder;

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    fn with_taskbar_icon(self, taskbar_icon: Option<Icon>) -> WindowBuilder;

    /// This sets `WS_EX_NOREDIRECTIONBITMAP`.
    fn with_no_redirection_bitmap(self, flag: bool) -> WindowBuilder;
}

impl WindowBuilderExtWindows for WindowBuilder {
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
    fn with_no_redirection_bitmap(mut self, flag: bool) -> WindowBuilder {
        self.platform_specific.no_redirection_bitmap = flag;
        self
    }
}

/// Additional methods on `MonitorHandle` that are specific to Windows.
pub trait MonitorHandleExtWindows {
    /// Returns the name of the monitor adapter specific to the Win32 API.
    fn native_id(&self) -> String;

    /// Returns the handle of the monitor - `HMONITOR`.
    fn hmonitor(&self) -> *mut c_void;
}

impl MonitorHandleExtWindows for MonitorHandle {
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
pub trait DeviceIdExtWindows {
    /// Returns an identifier that persistently refers to this specific device.
    ///
    /// Will return `None` if the device is no longer available.
    fn get_persistent_identifier(&self) -> Option<String>;
}

impl DeviceIdExtWindows for DeviceId {
    #[inline]
    fn get_persistent_identifier(&self) -> Option<String> {
        self.0.get_persistent_identifier()
    }
}
