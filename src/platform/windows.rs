use std::{ffi::c_void, path::Path};

use crate::{
    dpi::PhysicalSize,
    event::DeviceId,
    event_loop::EventLoopBuilder,
    monitor::MonitorHandle,
    window::{BadIcon, Icon, Window, WindowBuilder},
};

/// Window Handle type used by Win32 API
pub type HWND = isize;
/// Menu Handle type used by Win32 API
pub type HMENU = isize;
/// Monitor Handle type used by Win32 API
pub type HMONITOR = isize;

/// Describes a color used by Windows
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Color(u32);

impl Color {
    /// Use the system's default color
    pub const SYSTEM_DEFAULT: Color = Color(0xFFFFFFFF);

    //Special constant only valid for the window border and therefore modeled using Option<Color> for user facing code
    const NONE: Color = Color(0xFFFFFFFE);

    /// Create a new color from the given RGB values
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::SYSTEM_DEFAULT
    }
}

/// Describes how the corners of a window should look like.
///
/// For a detailed explanation, see [`DWM_WINDOW_CORNER_PREFERENCE docs`].
///
/// [`DWM_WINDOW_CORNER_PREFERENCE docs`]: https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_window_corner_preference
#[repr(i32)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CornerPreference {
    /// Corresponds to `DWMWCP_DEFAULT`.
    ///
    /// Let the system decide when to round window corners.
    #[default]
    Default = 0,

    /// Corresponds to `DWMWCP_DONOTROUND`.
    ///
    /// Never round window corners.
    DoNotRound = 1,

    /// Corresponds to `DWMWCP_ROUND`.
    ///
    /// Round the corners, if appropriate.
    Round = 2,

    /// Corresponds to `DWMWCP_ROUNDSMALL`.
    ///
    /// Round the corners if appropriate, with a small radius.
    RoundSmall = 3,
}

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

    /// A callback to be executed before dispatching a win32 message to the window procedure.
    /// Return true to disable winit's internal message dispatching.
    ///
    /// # Example
    ///
    /// ```
    /// # use windows_sys::Win32::UI::WindowsAndMessaging::{ACCEL, CreateAcceleratorTableW, TranslateAcceleratorW, DispatchMessageW, TranslateMessage, MSG};
    /// use winit::event_loop::EventLoopBuilder;
    /// #[cfg(target_os = "windows")]
    /// use winit::platform::windows::EventLoopBuilderExtWindows;
    ///
    /// let mut builder = EventLoopBuilder::new();
    /// #[cfg(target_os = "windows")]
    /// builder.with_msg_hook(|msg|{
    ///     let msg = msg as *const MSG;
    /// #   let accels: Vec<ACCEL> = Vec::new();
    ///     let translated = unsafe {
    ///         TranslateAcceleratorW(
    ///             (*msg).hwnd,
    ///             CreateAcceleratorTableW(accels.as_ptr() as _, 1),
    ///             msg,
    ///         ) == 1
    ///     };
    ///     translated
    /// });
    /// ```
    fn with_msg_hook<F>(&mut self, callback: F) -> &mut Self
    where
        F: FnMut(*const c_void) -> bool + 'static;
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
    fn with_msg_hook<F>(&mut self, callback: F) -> &mut Self
    where
        F: FnMut(*const c_void) -> bool + 'static,
    {
        self.platform_specific.msg_hook = Some(Box::new(callback));
        self
    }
}

/// Additional methods on `Window` that are specific to Windows.
pub trait WindowExtWindows {
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

    /// Whether to show or hide the window icon in the taskbar.
    fn set_skip_taskbar(&self, skip: bool);

    /// Shows or hides the background drop shadow for undecorated windows.
    ///
    /// Enabling the shadow causes a thin 1px line to appear on the top of the window.
    fn set_undecorated_shadow(&self, shadow: bool);

    /// Sets the color of the window border.
    ///
    /// Supported starting with Windows 11 Build 22000.
    fn set_border_color(&self, color: Option<Color>);

    /// Sets the background color of the title bar.
    ///
    /// Supported starting with Windows 11 Build 22000.
    fn set_title_background_color(&self, color: Option<Color>);

    /// Sets the color of the window title.
    ///
    /// Supported starting with Windows 11 Build 22000.
    fn set_title_text_color(&self, color: Color);

    /// Sets the preferred style of the window corners.
    ///
    /// Supported starting with Windows 11 Build 22000.
    fn set_corner_preference(&self, preference: CornerPreference);
}

impl WindowExtWindows for Window {
    #[inline]
    fn set_enable(&self, enabled: bool) {
        self.window.set_enable(enabled)
    }

    #[inline]
    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>) {
        self.window.set_taskbar_icon(taskbar_icon)
    }

    #[inline]
    fn set_skip_taskbar(&self, skip: bool) {
        self.window.set_skip_taskbar(skip)
    }

    #[inline]
    fn set_undecorated_shadow(&self, shadow: bool) {
        self.window.set_undecorated_shadow(shadow)
    }

    #[inline]
    fn set_border_color(&self, color: Option<Color>) {
        self.window.set_border_color(color.unwrap_or(Color::NONE))
    }

    #[inline]
    fn set_title_background_color(&self, color: Option<Color>) {
        // The windows docs don't mention NONE as a valid options but it works in practice and is useful
        // to circumvent the Windows option "Show accent color on title bars and window borders"
        self.window
            .set_title_background_color(color.unwrap_or(Color::NONE))
    }

    #[inline]
    fn set_title_text_color(&self, color: Color) {
        self.window.set_title_text_color(color)
    }

    #[inline]
    fn set_corner_preference(&self, preference: CornerPreference) {
        self.window.set_corner_preference(preference)
    }
}

/// Additional methods on `WindowBuilder` that are specific to Windows.
#[allow(rustdoc::broken_intra_doc_links)]
pub trait WindowBuilderExtWindows {
    /// Set an owner to the window to be created. Can be used to create a dialog box, for example.
    /// This only works when [`WindowBuilder::with_parent_window`] isn't called or set to `None`.
    /// Can be used in combination with [`WindowExtWindows::set_enable(false)`](WindowExtWindows::set_enable)
    /// on the owner window to create a modal dialog box.
    ///
    /// From MSDN:
    /// - An owned window is always above its owner in the z-order.
    /// - The system automatically destroys an owned window when its owner is destroyed.
    /// - An owned window is hidden when its owner is minimized.
    ///
    /// For more information, see <https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#owned-windows>
    fn with_owner_window(self, parent: HWND) -> Self;

    /// Sets a menu on the window to be created.
    ///
    /// Parent and menu are mutually exclusive; a child window cannot have a menu!
    ///
    /// The menu must have been manually created beforehand with [`CreateMenu`] or similar.
    ///
    /// Note: Dark mode cannot be supported for win32 menus, it's simply not possible to change how the menus look.
    /// If you use this, it is recommended that you combine it with `with_theme(Some(Theme::Light))` to avoid a jarring effect.
    ///
    #[cfg_attr(
        platform_windows,
        doc = "[`CreateMenu`]: windows_sys::Win32::UI::WindowsAndMessaging::CreateMenu"
    )]
    #[cfg_attr(
        not(platform_windows),
        doc = "[`CreateMenu`]: #only-available-on-windows"
    )]
    fn with_menu(self, menu: HMENU) -> Self;

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    fn with_taskbar_icon(self, taskbar_icon: Option<Icon>) -> Self;

    /// This sets `WS_EX_NOREDIRECTIONBITMAP`.
    fn with_no_redirection_bitmap(self, flag: bool) -> Self;

    /// Enables or disables drag and drop support (enabled by default). Will interfere with other crates
    /// that use multi-threaded COM API (`CoInitializeEx` with `COINIT_MULTITHREADED` instead of
    /// `COINIT_APARTMENTTHREADED`) on the same thread. Note that winit may still attempt to initialize
    /// COM API regardless of this option. Currently only fullscreen mode does that, but there may be more in the future.
    /// If you need COM API with `COINIT_MULTITHREADED` you must initialize it before calling any winit functions.
    /// See <https://docs.microsoft.com/en-us/windows/win32/api/objbase/nf-objbase-coinitialize#remarks> for more information.
    fn with_drag_and_drop(self, flag: bool) -> Self;

    /// Whether show or hide the window icon in the taskbar.
    fn with_skip_taskbar(self, skip: bool) -> Self;

    /// Customize the window class name.
    fn with_class_name<S: Into<String>>(self, class_name: S) -> Self;

    /// Shows or hides the background drop shadow for undecorated windows.
    ///
    /// The shadow is hidden by default.
    /// Enabling the shadow causes a thin 1px line to appear on the top of the window.
    fn with_undecorated_shadow(self, shadow: bool) -> Self;

    /// This sets or removes `WS_CLIPCHILDREN` style.
    fn with_clip_children(self, flag: bool) -> Self;

    /// Sets the color of the window border.
    ///
    /// Supported starting with Windows 11 Build 22000.
    fn with_border_color(self, color: Option<Color>) -> Self;

    /// Sets the background color of the title bar.
    ///
    /// Supported starting with Windows 11 Build 22000.
    fn with_title_background_color(self, color: Option<Color>) -> Self;

    /// Sets the color of the window title.
    ///
    /// Supported starting with Windows 11 Build 22000.
    fn with_title_text_color(self, color: Color) -> Self;

    /// Sets the preferred style of the window corners.
    ///
    /// Supported starting with Windows 11 Build 22000.
    fn with_corner_preference(self, corners: CornerPreference) -> Self;
}

impl WindowBuilderExtWindows for WindowBuilder {
    #[inline]
    fn with_owner_window(mut self, parent: HWND) -> Self {
        self.window.platform_specific.owner = Some(parent);
        self
    }

    #[inline]
    fn with_menu(mut self, menu: HMENU) -> Self {
        self.window.platform_specific.menu = Some(menu);
        self
    }

    #[inline]
    fn with_taskbar_icon(mut self, taskbar_icon: Option<Icon>) -> Self {
        self.window.platform_specific.taskbar_icon = taskbar_icon;
        self
    }

    #[inline]
    fn with_no_redirection_bitmap(mut self, flag: bool) -> Self {
        self.window.platform_specific.no_redirection_bitmap = flag;
        self
    }

    #[inline]
    fn with_drag_and_drop(mut self, flag: bool) -> Self {
        self.window.platform_specific.drag_and_drop = flag;
        self
    }

    #[inline]
    fn with_skip_taskbar(mut self, skip: bool) -> Self {
        self.window.platform_specific.skip_taskbar = skip;
        self
    }

    #[inline]
    fn with_class_name<S: Into<String>>(mut self, class_name: S) -> Self {
        self.window.platform_specific.class_name = class_name.into();
        self
    }

    #[inline]
    fn with_undecorated_shadow(mut self, shadow: bool) -> Self {
        self.window.platform_specific.decoration_shadow = shadow;
        self
    }

    #[inline]
    fn with_clip_children(mut self, flag: bool) -> Self {
        self.window.platform_specific.clip_children = flag;
        self
    }

    #[inline]
    fn with_border_color(mut self, color: Option<Color>) -> Self {
        self.window.platform_specific.border_color = Some(color.unwrap_or(Color::NONE));
        self
    }

    #[inline]
    fn with_title_background_color(mut self, color: Option<Color>) -> Self {
        self.window.platform_specific.title_background_color = Some(color.unwrap_or(Color::NONE));
        self
    }

    #[inline]
    fn with_title_text_color(mut self, color: Color) -> Self {
        self.window.platform_specific.title_text_color = Some(color);
        self
    }

    #[inline]
    fn with_corner_preference(mut self, corners: CornerPreference) -> Self {
        self.window.platform_specific.corner_preference = Some(corners);
        self
    }
}

/// Additional methods on `MonitorHandle` that are specific to Windows.
pub trait MonitorHandleExtWindows {
    /// Returns the name of the monitor adapter specific to the Win32 API.
    fn native_id(&self) -> String;

    /// Returns the handle of the monitor - `HMONITOR`.
    fn hmonitor(&self) -> HMONITOR;
}

impl MonitorHandleExtWindows for MonitorHandle {
    #[inline]
    fn native_id(&self) -> String {
        self.inner.native_identifier()
    }

    #[inline]
    fn hmonitor(&self) -> HMONITOR {
        self.inner.hmonitor()
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
    fn from_resource(ordinal: u16, size: Option<PhysicalSize<u32>>) -> Result<Self, BadIcon>;
}

impl IconExtWindows for Icon {
    fn from_path<P: AsRef<Path>>(
        path: P,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        let win_icon = crate::platform_impl::WinIcon::from_path(path, size)?;
        Ok(Icon { inner: win_icon })
    }

    fn from_resource(ordinal: u16, size: Option<PhysicalSize<u32>>) -> Result<Self, BadIcon> {
        let win_icon = crate::platform_impl::WinIcon::from_resource(ordinal, size)?;
        Ok(Icon { inner: win_icon })
    }
}
