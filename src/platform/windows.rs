#![cfg(target_os = "windows")]

use std::os::raw::c_void;

use libc;
use winapi::shared::windef::HWND;
use winapi::shared::minwindef::{UINT, WPARAM, LPARAM, LRESULT};

use std::fmt;

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

/// Unprocessed window event that are specific to Windows.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OsSpecificWindowEvent {
    pub(crate) window: HWND,
    /// The message identifier of the window event (`WM_*`).
    pub message: UINT,
    /// The first parameter of the window event (`WPARAM`).
    pub wparam: WPARAM,
    /// The second parameter of the window event(`LPARAM`).
    pub lparam: LPARAM,
    pub(crate) retval: *mut Option<LRESULT>,
}

impl OsSpecificWindowEvent {
    /// Marks the current event processed, returning the specified value
    /// as the result of the window procedure to the system.
    /// How the value is interpreted is dependent on the message's type.
    /// The next window procedure (or subclass window
    /// procedure) won't be called for this message.
    ///
    /// Calling this function outside this event's call to the closure passed to
    /// [`EventLoop::run`](../../event_loop/struct.EventLoop.html#method.run)
    /// will result in undefined behavior.
    #[inline]
    pub unsafe fn set_reply(&self, val: isize) {
        *self.retval = Some(val);
    }

    /// Immediately calls the next window procedure(or subclass window
    /// procedure), and maps its return value through the user-passed closure.
    /// How the value is interpreted is dependent on the message's type.
    ///
    /// This function must not be called more than once for each message.
    ///
    /// Calling this function outside this event's call to the closure passed to
    /// [`EventLoop::run`](../../event_loop/struct.EventLoop.html#method.run)
    /// will result in undefined behavior.
    #[inline]
    pub unsafe fn set_overriden_reply(&self, f: impl FnOnce(isize) -> isize) {
        use winapi::um::commctrl;
        let retval = f(commctrl::DefSubclassProc(self.window, self.message, self.wparam, self.lparam));
        *self.retval = Some(retval);
    }
}


impl fmt::Debug for OsSpecificWindowEvent {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        fmtr.pad("OsSpecificWindowEvent { .. }")
    }
}
