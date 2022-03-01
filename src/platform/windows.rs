#![cfg(target_os = "windows")]

use std::os::raw::c_void;
use std::path::Path;

use winapi::shared::minwindef::WORD;
use winapi::shared::windef::{HMENU, HWND};

use crate::{
    dpi::PhysicalSize,
    event::DeviceId,
    event_loop::EventLoopBuilder,
    monitor::MonitorHandle,
    platform_impl::{Parent, WinIcon},
    window::{BadIcon, Icon, Theme, Window, WindowBuilder},
};

/// Additional methods on `EventLoop` that are specific to Windows.
pub trait EventLoopBuilderExtWindows {
    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    ///
    /// # `Window` caveats
    ///
    /// Note that any `Window` created on the new thread will be destroyed when the thread
    /// terminates. Attempting to use a `Window` after its parent thread terminates has
    /// unspecified, although explicitly not undefined, behavior.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;

    /// Whether to enable process-wide DPI awareness.
    ///
    /// By default, `winit` will attempt to enable process-wide DPI awareness. If
    /// that's undesirable, you can disable it with this function.
    ///
    /// # Example
    ///
    /// Disable process-wide DPI awareness.
    ///
    /// ```
    /// use winit::event_loop::EventLoopBuilder;
    /// #[cfg(target_os = "windows")]
    /// use winit::platform::windows::EventLoopBuilderExtWindows;
    ///
    /// let mut builder = EventLoopBuilder::new();
    /// #[cfg(target_os = "windows")]
    /// builder.with_dpi_aware(false);
    /// # if false { // We can't test this part
    /// let event_loop = builder.build();
    /// # }
    /// ```
    fn with_dpi_aware(&mut self, dpi_aware: bool) -> &mut Self;

    /// A callback, executed before dispatching a win32 message to the window procedure, where you
    /// can call [`TranslateAcceleratorW`](https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-translateacceleratorw) to
    /// processes accelerator keys for menu commands.
    ///
    /// The callback return value is used as an indicator to whether the message
    /// should be translated using [`TranslateMessage`](https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-translatemessage)
    ///
    /// # Example
    ///
    /// ```no_run
    /// thread_local! {
    ///     static ACCELS:Vec<ACCEL> = {
    ///         unsafe {
    ///             vec![ACCEL {
    ///                 cmd: 554,
    ///                 key: VkKeyScanW('a' as u16) as _  ,
    ///                 fVirt: VK_CONTROL as _,
    ///             }]
    ///         }
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let event_loop = EventLoopBuilder::new()
    ///         .with_translate_accel_callback(Box::new(|mut m| unsafe {
    ///             let mut translated = false;
    ///             ACCELS.with(|accels| {
    ///                 translated = TranslateAcceleratorW(
    ///                     m.hwnd,
    ///                     CreateAcceleratorTableW(accels.as_ptr() as _, 1),
    ///                     &mut m,
    ///                 ) == 1;
    ///             });
    ///             translated
    ///         }))
    ///         .build();
    /// }
    /// ```
    fn with_translate_accel_callback(
        &mut self,
        callback: Box<dyn FnMut(winapi::um::winuser::MSG) -> bool>,
    ) -> &mut Self;
}

impl<T> EventLoopBuilderExtWindows for EventLoopBuilder<T> {
    #[inline]
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.platform_specific.any_thread = any_thread;
        self
    }

    #[inline]
    fn with_dpi_aware(&mut self, dpi_aware: bool) -> &mut Self {
        self.platform_specific.dpi_aware = dpi_aware;
        self
    }

    #[inline]
    fn with_translate_accel_callback(
        &mut self,
        callback: Box<dyn FnMut(winapi::um::winuser::MSG) -> bool>,
    ) -> &mut Self {
        self.platform_specific.translate_accel_callback = Some(callback);
        self
    }
}

/// Additional methods on `Window` that are specific to Windows.
pub trait WindowExtWindows {
    /// Returns the HINSTANCE of the window
    fn hinstance(&self) -> *mut c_void;
    /// Returns the native handle that is used by this window.
    ///
    /// The pointer will become invalid when the native window was destroyed.
    fn hwnd(&self) -> *mut c_void;

    /// Enables or disables mouse and keyboard input to the specified window.
    ///
    /// A window must be enabled before it can be activated.
    /// If an application has create a modal dialog box by disabling its owner window
    /// (as described in [`WindowBuilderExtWindows::with_owner_window`]), the application must enable
    /// the owner window before destroying the dialog box.
    /// Otherwise, another window will receive the keyboard focus and be activated.
    ///
    /// If a child window is disabled, it is ignored when the system tries to determine which
    /// window should receive mouse messages.
    ///
    /// For more information, see <https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enablewindow#remarks>
    /// and <https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#disabled-windows>
    fn set_enable(&self, enabled: bool);

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>);

    /// Returns the current window theme.
    fn theme(&self) -> Theme;
}

impl WindowExtWindows for Window {
    #[inline]
    fn hinstance(&self) -> *mut c_void {
        self.window.hinstance() as *mut _
    }

    #[inline]
    fn hwnd(&self) -> *mut c_void {
        self.window.hwnd() as *mut _
    }

    #[inline]
    fn set_enable(&self, enabled: bool) {
        self.window.set_enable(enabled)
    }

    #[inline]
    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>) {
        self.window.set_taskbar_icon(taskbar_icon)
    }

    #[inline]
    fn theme(&self) -> Theme {
        self.window.theme()
    }
}

/// Additional methods on `WindowBuilder` that are specific to Windows.
pub trait WindowBuilderExtWindows {
    /// Sets a parent to the window to be created.
    ///
    /// A child window has the WS_CHILD style and is confined to the client area of its parent window.
    ///
    /// For more information, see <https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#child-windows>
    fn with_parent_window(self, parent: HWND) -> WindowBuilder;

    /// Set an owner to the window to be created. Can be used to create a dialog box, for example.
    /// Can be used in combination with [`WindowExtWindows::set_enable(false)`](WindowExtWindows::set_enable)
    /// on the owner window to create a modal dialog box.
    ///
    /// From MSDN:
    /// - An owned window is always above its owner in the z-order.
    /// - The system automatically destroys an owned window when its owner is destroyed.
    /// - An owned window is hidden when its owner is minimized.
    ///
    /// For more information, see <https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#owned-windows>
    fn with_owner_window(self, parent: HWND) -> WindowBuilder;

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
    /// See <https://docs.microsoft.com/en-us/windows/win32/api/objbase/nf-objbase-coinitialize#remarks> for more information.
    fn with_drag_and_drop(self, flag: bool) -> WindowBuilder;

    /// Forces a theme or uses the system settings if `None` was provided.
    fn with_theme(self, theme: Option<Theme>) -> WindowBuilder;
}

impl WindowBuilderExtWindows for WindowBuilder {
    #[inline]
    fn with_parent_window(mut self, parent: HWND) -> WindowBuilder {
        self.platform_specific.parent = Parent::ChildOf(parent);
        self
    }

    #[inline]
    fn with_owner_window(mut self, parent: HWND) -> WindowBuilder {
        self.platform_specific.parent = Parent::OwnedBy(parent);
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
