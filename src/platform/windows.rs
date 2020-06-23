#![cfg(target_os = "windows")]

use std::os::raw::c_void;
use std::path::Path;

use libc;
use winapi::shared::minwindef::WORD;
use winapi::shared::windef::HWND;

use crate::{
    dpi::PhysicalSize,
    event::{DeviceId, Event},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    monitor::MonitorHandle,
    platform_impl::{
        EventLoop as WindowsEventLoop, EventLoopEmbedded as WindowsEventLoopEmbedded, WinIcon,
    },
    window::{BadIcon, Icon, Window, WindowBuilder},
};

pub struct EventLoopEmbedded<'a, T: 'static> {
    p: WindowsEventLoopEmbedded<'a, T>,
}

/// Additional methods on `EventLoop` that are specific to Windows.
pub trait EventLoopExtWindows {
    type UserEvent;
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

    /// Initialize an event loop that can run through somebody else's event pump.
    ///
    /// This does *not* dispatch events without external assistance! Other code must be running a
    /// [Win32 message loop](https://docs.microsoft.com/en-us/windows/win32/learnwin32/window-messages),
    /// and the `event_handler` closure will be called while the `EventLoopEmbedded` is in scope.
    /// The loop can come from any code that calls the native Win32 message loop functions - for
    /// example, this could be used to embed a Winit message loop in an SDL or GLFW application, or
    /// create a DAW plugin.
    ///
    /// TODO: REWRITE `exit_requested` and `resume_panic_if_necessary` as trait functions.
    fn run_embedded<'a, F>(self, event_handler: F) -> EventLoopEmbedded<'a, Self::UserEvent>
    where
        F: 'a
            + FnMut(
                Event<'_, Self::UserEvent>,
                &EventLoopWindowTarget<Self::UserEvent>,
                &mut ControlFlow,
            );
}

impl<T> EventLoopExtWindows for EventLoop<T> {
    type UserEvent = T;
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

    fn run_embedded<'a, F>(self, event_handler: F) -> EventLoopEmbedded<'a, Self::UserEvent>
    where
        F: 'a
            + FnMut(
                Event<'_, Self::UserEvent>,
                &EventLoopWindowTarget<Self::UserEvent>,
                &mut ControlFlow,
            ),
    {
        EventLoopEmbedded {
            p: self.event_loop.run_embedded(event_handler),
        }
    }
}

impl<T> EventLoopEmbedded<'_, T> {
    pub fn exit_requested(&self) -> bool {
        self.p.exit_requested()
    }

    pub fn resume_panic_if_necessary(&self) {
        self.p.resume_panic_if_necessary()
    }
}

/// Additional methods on `EventLoopWindowTarget` that are specific to Windows.
pub trait EventLoopWindowTargetExtWindows {
    /// Schedule a closure to be invoked after the current event handler returns.
    ///
    /// This is useful if you're calling one of the Windows API's many *modal functions*. Modal
    /// functions take over control of the event loop for the duration of their execution, and don't
    /// return control flow to the caller until the operation they perform has been completed.
    /// They're typically used for popup windows that the user must click through to continue using
    /// the program - the [`MessageBox`](https://docs.microsoft.com/en-us/windows/win32/dlgbox/using-dialog-boxes#displaying-a-message-box)
    /// function is a good example of this.
    ///
    /// The reason this function is necessary is that, if you call a modal function inside of the
    /// standard Winit event handler closure, Winit cannot dispatch OS events to that closure
    /// while the modal loop is running since the closure is being borrowed by the closure
    /// invocation that called the modal function. This function sidesteps that issue by allowing
    /// you to call modal functions outside the scope of the normal event handler function, which
    /// prevents the double-borrowing and allows event loop execution to continue as normal.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// event_loop_window_target.schedule_modal_fn(move || unsafe {
    ///     println!("\n\t\tstart modal loop\n");
    ///
    ///     let msg_box_id = winuser::MessageBoxA(
    ///         hwnd as _,
    ///         "Please press Yes or No\0".as_ptr() as *const _,
    ///         "Dialog Box\0".as_ptr() as *const _,
    ///         winuser::MB_ICONEXCLAMATION | winuser::MB_YESNO
    ///     );
    ///
    ///     println!("\n\t\tend modal loop\n");
    ///
    ///     if msg_box_id == winuser::IDYES {
    ///         println!("Yes pressed!");
    ///     } else {
    ///         println!("No pressed!");
    ///     }
    /// });
    /// ```
    ///
    /// See also the `win32_modal_dialog.rs` example.
    fn schedule_modal_fn(&self, f: impl 'static + FnOnce());
}

impl<T> EventLoopWindowTargetExtWindows for EventLoopWindowTarget<T> {
    fn schedule_modal_fn(&self, f: impl 'static + FnOnce()) {
        self.p.schedule_modal_fn(f);
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

    /// Whether the system theme is currently Windows 10's "Dark Mode".
    fn is_dark_mode(&self) -> bool;
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
    fn is_dark_mode(&self) -> bool {
        self.window.is_dark_mode()
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
