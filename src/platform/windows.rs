#![cfg(target_os = "windows")]

use std::os::raw::c_void;
use std::path::Path;

use libc;
use winapi::shared::minwindef::WORD;
use winapi::shared::windef::{HMENU, HWND};
use winapi::um::winuser::SetFocus;

use crate::{
    dpi::PhysicalSize,
    event::DeviceId,
    event_loop::EventLoop,
    monitor::MonitorHandle,
    platform_impl::{EventLoop as WindowsEventLoop, WinIcon},
    window::{BadIcon, Icon, Theme, Window, WindowBuilder},
};

/// Additional methods on `EventLoop` that are specific to Windows.
pub trait EventLoopExtWindows {
    /// Creates an event loop off of the main thread.
    ///
    /// # `Window` caveats
    ///
    /// Note that any `Window` created on the new thread will be destroyed when the thread
    /// terminates. Attempting to use a `Window` after its parent thread terminates has
    /// unspecified, although explicitly not undefined, behavior.
    fn new_any_thread() -> Self
    where
        Self: Sized;

    /// By default, winit on Windows will attempt to enable process-wide DPI awareness. If that's
    /// undesirable, you can create an `EventLoop` using this function instead.
    fn new_dpi_unaware() -> Self
    where
        Self: Sized;

    /// Creates a DPI-unaware event loop off of the main thread.
    ///
    /// The `Window` caveats in [`new_any_thread`](EventLoopExtWindows::new_any_thread) also apply here.
    fn new_dpi_unaware_any_thread() -> Self
    where
        Self: Sized;
}

impl<T> EventLoopExtWindows for EventLoop<T> {
    #[inline]
    fn new_any_thread() -> Self {
        EventLoop {
            event_loop: WindowsEventLoop::new_any_thread(),
            _marker: ::std::marker::PhantomData,
        }
    }

    #[inline]
    fn new_dpi_unaware() -> Self {
        EventLoop {
            event_loop: WindowsEventLoop::new_dpi_unaware(),
            _marker: ::std::marker::PhantomData,
        }
    }

    #[inline]
    fn new_dpi_unaware_any_thread() -> Self {
        EventLoop {
            event_loop: WindowsEventLoop::new_dpi_unaware_any_thread(),
            _marker: ::std::marker::PhantomData,
        }
    }
}

/// Additional methods on `Window` that are specific to Windows.
pub trait WindowExtWindows {
    /// Returns the HINSTANCE of the window
    fn hinstance(&self) -> *mut libc::c_void;
    /// Returns the native handle that is used by this window.
    ///
    /// The pointer will become invalid when the native window was destroyed.
    fn hwnd(&self) -> *mut libc::c_void;

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>);

    /// Returns the current window theme.
    fn theme(&self) -> Theme;

    /// Focus the window, and moves the window to the front of the screen list
    fn focus_and_front(&self);
}

impl WindowExtWindows for Window {
    #[inline]
    fn hinstance(&self) -> *mut libc::c_void {
        self.window.hinstance() as *mut _
    }

    #[inline]
    fn hwnd(&self) -> *mut libc::c_void {
        self.window.hwnd() as *mut _
    }

    #[inline]
    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>) {
        self.window.set_taskbar_icon(taskbar_icon)
    }

    #[inline]
    fn theme(&self) -> Theme {
        self.window.theme()
    }

    #[inline]
    fn focus_and_front(&self) {
        unsafe {
            SetFocus(self.window.hwnd());
        }
    }
}

/// Additional methods on `WindowBuilder` that are specific to Windows.
pub trait WindowBuilderExtWindows {
    /// Sets a parent to the window to be created.
    fn with_parent_window(self, parent: HWND) -> WindowBuilder;

    /// Sets a menu on the window to be created.
    ///
    /// Parent and menu are mutually exclusive; a child window cannot have a menu!
    ///
    /// The menu must have been manually created beforehand with [`winapi::um::winuser::CreateMenu`] or similar.
    ///
    /// Note: Dark mode cannot be supported for win32 menus, it's simply not possible to change how the menus look.
    /// If you use this, it is recommended that you combine it with `with_theme(Some(Theme::Light))` to avoid a jarring effect.
    fn with_menu(self, menu: HMENU) -> WindowBuilder;

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    fn with_taskbar_icon(self, taskbar_icon: Option<Icon>) -> WindowBuilder;

    /// This sets `WS_EX_NOREDIRECTIONBITMAP`.
    fn with_no_redirection_bitmap(self, flag: bool) -> WindowBuilder;

    /// Enables or disables drag and drop support (enabled by default). Will interfere with other crates
    /// that use multi-threaded COM API (`CoInitializeEx` with `COINIT_MULTITHREADED` instead of
    /// `COINIT_APARTMENTTHREADED`) on the same thread. Note that winit may still attempt to initialize
    /// COM API regardless of this option. Currently only fullscreen mode does that, but there may be more in the future.
    /// If you need COM API with `COINIT_MULTITHREADED` you must initialize it before calling any winit functions.
    /// See https://docs.microsoft.com/en-us/windows/win32/api/objbase/nf-objbase-coinitialize#remarks for more information.
    fn with_drag_and_drop(self, flag: bool) -> WindowBuilder;

    /// Forces a theme or uses the system settings if `None` was provided.
    fn with_theme(self, theme: Option<Theme>) -> WindowBuilder;
}

impl WindowBuilderExtWindows for WindowBuilder {
    #[inline]
    fn with_parent_window(mut self, parent: HWND) -> WindowBuilder {
        self.platform_specific.parent = Some(parent);
        self
    }

    #[inline]
    fn with_menu(mut self, menu: HMENU) -> WindowBuilder {
        self.platform_specific.menu = Some(menu);
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

    #[inline]
    fn with_drag_and_drop(mut self, flag: bool) -> WindowBuilder {
        self.platform_specific.drag_and_drop = flag;
        self
    }

    #[inline]
    fn with_theme(mut self, theme: Option<Theme>) -> WindowBuilder {
        self.platform_specific.preferred_theme = theme;
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
        self.inner.native_identifier()
    }

    #[inline]
    fn hmonitor(&self) -> *mut c_void {
        self.inner.hmonitor() as *mut _
    }
}

/// Additional methods on `DeviceId` that are specific to Windows.
pub trait DeviceIdExtWindows {
    /// Returns an identifier that persistently refers to this specific device.
    ///
    /// Will return `None` if the device is no longer available.
    fn persistent_identifier(&self) -> Option<String>;
}

impl DeviceIdExtWindows for DeviceId {
    #[inline]
    fn persistent_identifier(&self) -> Option<String> {
        self.0.persistent_identifier()
    }
}

/// Additional methods on `Icon` that are specific to Windows.
pub trait IconExtWindows: Sized {
    /// Create an icon from a file path.
    ///
    /// Specify `size` to load a specific icon size from the file, or `None` to load the default
    /// icon size from the file.
    ///
    /// In cases where the specified size does not exist in the file, Windows may perform scaling
    /// to get an icon of the desired size.
    fn from_path<P: AsRef<Path>>(path: P, size: Option<PhysicalSize<u32>>)
        -> Result<Self, BadIcon>;

    /// Create an icon from a resource embedded in this executable or library.
    ///
    /// Specify `size` to load a specific icon size from the file, or `None` to load the default
    /// icon size from the file.
    ///
    /// In cases where the specified size does not exist in the file, Windows may perform scaling
    /// to get an icon of the desired size.
    fn from_resource(ordinal: WORD, size: Option<PhysicalSize<u32>>) -> Result<Self, BadIcon>;
}

impl IconExtWindows for Icon {
    fn from_path<P: AsRef<Path>>(
        path: P,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        let win_icon = WinIcon::from_path(path, size)?;
        Ok(Icon { inner: win_icon })
    }

    fn from_resource(ordinal: WORD, size: Option<PhysicalSize<u32>>) -> Result<Self, BadIcon> {
        let win_icon = WinIcon::from_resource(ordinal, size)?;
        Ok(Icon { inner: win_icon })
    }
}
