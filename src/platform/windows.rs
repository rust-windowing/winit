//! # Windows
//!
//! The supported OS version is Windows 7 or higher, though Windows 10 is
//! tested regularly.
use std::borrow::Borrow;
use std::ffi::c_void;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(windows_platform)]
use windows_sys::Win32::Foundation::HANDLE;
use winit_core::window::PlatformWindowAttributes;

use crate::dpi::PhysicalSize;
use crate::event::DeviceId;
use crate::event_loop::EventLoopBuilder;
use crate::icon::{BadIcon, Icon};
use crate::platform_impl::RaiiIcon;
use crate::window::Window;

/// Window Handle type used by Win32 API
pub type HWND = *mut c_void;
/// Menu Handle type used by Win32 API
pub type HMENU = *mut c_void;
/// Monitor Handle type used by Win32 API
pub type HMONITOR = *mut c_void;

/// Describes a system-drawn backdrop material of a window.
///
/// For a detailed explanation, see [`DWM_SYSTEMBACKDROP_TYPE docs`].
///
/// [`DWM_SYSTEMBACKDROP_TYPE docs`]: https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BackdropType {
    /// Corresponds to `DWMSBT_AUTO`.
    ///
    /// Usually draws a default backdrop effect on the title bar.
    #[default]
    Auto = 0,

    /// Corresponds to `DWMSBT_NONE`.
    None = 1,

    /// Corresponds to `DWMSBT_MAINWINDOW`.
    ///
    /// Draws the Mica backdrop material.
    MainWindow = 2,

    /// Corresponds to `DWMSBT_TRANSIENTWINDOW`.
    ///
    /// Draws the Background Acrylic backdrop material.
    TransientWindow = 3,

    /// Corresponds to `DWMSBT_TABBEDWINDOW`.
    ///
    /// Draws the Alt Mica backdrop material.
    TabbedWindow = 4,
}

/// Describes a color used by Windows
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Color(u32);

impl Color {
    // Special constant only valid for the window border and therefore modeled using Option<Color>
    // for user facing code
    const NONE: Color = Color(0xfffffffe);
    /// Use the system's default color
    pub const SYSTEM_DEFAULT: Color = Color(0xffffffff);

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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

/// A wrapper around a [`Window`] that ignores thread-specific window handle limitations.
///
/// See [`WindowBorrowExtWindows::any_thread`] for more information.
#[derive(Clone, Debug)]
pub struct AnyThread<W: Window>(W);

impl<W: Window> AnyThread<W> {
    /// Get a reference to the inner window.
    #[inline]
    pub fn get_ref(&self) -> &dyn Window {
        &self.0
    }
}

impl<W: Window> Deref for AnyThread<W> {
    type Target = W;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<W: Window> rwh_06::HasWindowHandle for AnyThread<W> {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        // SAFETY: The top level user has asserted this is only used safely.
        unsafe { self.get_ref().window_handle_any_thread() }
    }
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
    /// use winit::event_loop::EventLoop;
    /// #[cfg(target_os = "windows")]
    /// use winit::platform::windows::EventLoopBuilderExtWindows;
    ///
    /// let mut builder = EventLoop::builder();
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
    /// use winit::event_loop::EventLoop;
    /// #[cfg(target_os = "windows")]
    /// use winit::platform::windows::EventLoopBuilderExtWindows;
    ///
    /// let mut builder = EventLoop::builder();
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

impl EventLoopBuilderExtWindows for EventLoopBuilder {
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
    /// (as described in [`WindowAttributesWindows::with_owner_window`]), the application must
    /// enable the owner window before destroying the dialog box.
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

    /// Sets system-drawn backdrop type.
    ///
    /// Requires Windows 11 build 22523+.
    fn set_system_backdrop(&self, backdrop_type: BackdropType);

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

    /// Turn window title bar on or off by setting `WS_CAPTION`.
    /// By default this is enabled. Note that fullscreen windows
    /// naturally do not have title bar.
    fn set_titlebar(&self, titlebar: bool);

    /// Gets the window's current titlebar state.
    ///
    /// Returns `true` when windows have a titlebar (server-side or by Winit).
    fn is_titlebar(&self) -> bool;

    /// Turn window top resize border on or off (for windows without a title bar).
    /// By default this is enabled.
    fn set_top_resize_border(&self, top_resize_border: bool);

    /// Gets the window's current top resize border state (for windows without a title bar).
    ///
    /// Returns `true` when windows have a top resize border.
    fn is_top_resize_border(&self) -> bool;

    /// Sets the preferred style of the window corners.
    ///
    /// Supported starting with Windows 11 Build 22000.
    fn set_corner_preference(&self, preference: CornerPreference);

    /// Get the raw window handle for this [`Window`] without checking for thread affinity.
    ///
    /// Window handles in Win32 have a property called "thread affinity" that ties them to their
    /// origin thread. Some operations can only happen on the window's origin thread, while others
    /// can be called from any thread. For example, [`SetWindowSubclass`] is not thread safe while
    /// [`GetDC`] is thread safe.
    ///
    /// In Rust terms, the window handle is `Send` sometimes but `!Send` other times.
    ///
    /// Therefore, in order to avoid confusing threading errors, [`Window`] only returns the
    /// window handle when the [`window_handle`] function is called from the thread that created
    /// the window. In other cases, it returns an [`Unavailable`] error.
    ///
    /// However in some cases you may already know that you are using the window handle for
    /// operations that are guaranteed to be thread-safe. In which case this function aims
    /// to provide an escape hatch so these functions are still accessible from other threads.
    ///
    /// # Safety
    ///
    /// It is the responsibility of the user to only pass the window handle into thread-safe
    /// Win32 APIs.
    ///
    /// [`SetWindowSubclass`]: https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-setwindowsubclass
    /// [`GetDC`]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdc
    /// [`Window`]: crate::window::Window
    /// [`window_handle`]: https://docs.rs/raw-window-handle/latest/raw_window_handle/trait.HasWindowHandle.html#tymethod.window_handle
    /// [`Unavailable`]: https://docs.rs/raw-window-handle/latest/raw_window_handle/enum.HandleError.html#variant.Unavailable
    ///
    /// ## Example
    ///
    /// ```no_run
    /// # use winit::window::Window;
    /// # fn scope(window: Box<dyn Window>) {
    /// use std::thread;
    ///
    /// use winit::platform::windows::WindowExtWindows;
    /// use winit::raw_window_handle::HasWindowHandle;
    ///
    /// // We can get the window handle on the current thread.
    /// let handle = window.window_handle().unwrap();
    ///
    /// // However, on another thread, we can't!
    /// thread::spawn(move || {
    ///     assert!(window.window_handle().is_err());
    ///
    ///     // We can use this function as an escape hatch.
    ///     let handle = unsafe { window.window_handle_any_thread().unwrap() };
    /// });
    /// # }
    /// ```
    unsafe fn window_handle_any_thread(
        &self,
    ) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError>;
}

impl WindowExtWindows for dyn Window + '_ {
    #[inline]
    fn set_enable(&self, enabled: bool) {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_enable(enabled)
    }

    #[inline]
    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>) {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_taskbar_icon(taskbar_icon)
    }

    #[inline]
    fn set_skip_taskbar(&self, skip: bool) {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_skip_taskbar(skip)
    }

    #[inline]
    fn set_undecorated_shadow(&self, shadow: bool) {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_undecorated_shadow(shadow)
    }

    #[inline]
    fn set_system_backdrop(&self, backdrop_type: BackdropType) {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_system_backdrop(backdrop_type)
    }

    #[inline]
    fn set_border_color(&self, color: Option<Color>) {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_border_color(color.unwrap_or(Color::NONE))
    }

    #[inline]
    fn set_title_background_color(&self, color: Option<Color>) {
        // The windows docs don't mention NONE as a valid options but it works in practice and is
        // useful to circumvent the Windows option "Show accent color on title bars and
        // window borders"
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_title_background_color(color.unwrap_or(Color::NONE))
    }

    #[inline]
    fn set_title_text_color(&self, color: Color) {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_title_text_color(color)
    }

    #[inline]
    fn set_titlebar(&self, titlebar: bool) {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_titlebar(titlebar)
    }

    #[inline]
    fn is_titlebar(&self) -> bool {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.is_titlebar()
    }

    #[inline]
    fn set_top_resize_border(&self, top_resize_border: bool) {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_top_resize_border(top_resize_border)
    }

    #[inline]
    fn is_top_resize_border(&self) -> bool {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.is_top_resize_border()
    }

    #[inline]
    fn set_corner_preference(&self, preference: CornerPreference) {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        window.set_corner_preference(preference)
    }

    unsafe fn window_handle_any_thread(
        &self,
    ) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let window = self.cast_ref::<crate::platform_impl::Window>().unwrap();
        unsafe {
            let handle = window.rwh_06_no_thread_check()?;

            // SAFETY: The handle is valid in this context.
            Ok(rwh_06::WindowHandle::borrow_raw(handle))
        }
    }
}

/// Additional methods for anything that dereference to [`Window`].
///
/// [`Window`]: crate::window::Window
pub trait WindowBorrowExtWindows: Borrow<dyn Window> + Sized {
    /// Create an object that allows accessing the inner window handle in a thread-unsafe way.
    ///
    /// It is possible to call [`window_handle_any_thread`] to get around Windows's thread
    /// affinity limitations. However, it may be desired to pass the [`Window`] into something
    /// that requires the [`HasWindowHandle`] trait, while ignoring thread affinity limitations.
    ///
    /// This function wraps anything that implements `Borrow<Window>` into a structure that
    /// uses the inner window handle as a mean of implementing [`HasWindowHandle`]. It wraps
    /// `Window`, `&Window`, `Arc<Window>`, and other reference types.
    ///
    /// # Safety
    ///
    /// It is the responsibility of the user to only pass the window handle into thread-safe
    /// Win32 APIs.
    ///
    /// [`Window`]: crate::window::Window
    /// [`HasWindowHandle`]: rwh_06::HasWindowHandle
    /// [`window_handle_any_thread`]: WindowExtWindows::window_handle_any_thread
    unsafe fn any_thread(self) -> AnyThread<Self>
    where
        Self: Window,
    {
        AnyThread(self)
    }
}

impl<W: Borrow<dyn Window> + Sized> WindowBorrowExtWindows for W {}

#[derive(Clone, Debug)]
pub struct WindowAttributesWindows {
    pub(crate) owner: Option<HWND>,
    pub(crate) menu: Option<HMENU>,
    pub(crate) taskbar_icon: Option<Icon>,
    pub(crate) no_redirection_bitmap: bool,
    pub(crate) drag_and_drop: bool,
    pub(crate) skip_taskbar: bool,
    pub(crate) class_name: String,
    pub(crate) decoration_shadow: bool,
    pub(crate) backdrop_type: BackdropType,
    pub(crate) clip_children: bool,
    pub(crate) border_color: Option<Color>,
    pub(crate) title_background_color: Option<Color>,
    pub(crate) title_text_color: Option<Color>,
    pub(crate) corner_preference: Option<CornerPreference>,
    pub(crate) titlebar: bool,
    pub(crate) top_resize_border: bool,
}

impl Default for WindowAttributesWindows {
    fn default() -> Self {
        Self {
            owner: None,
            menu: None,
            taskbar_icon: None,
            no_redirection_bitmap: false,
            drag_and_drop: true,
            skip_taskbar: false,
            class_name: "Window Class".to_string(),
            decoration_shadow: false,
            backdrop_type: BackdropType::default(),
            clip_children: true,
            border_color: None,
            title_background_color: None,
            title_text_color: None,
            corner_preference: None,
            titlebar: true,
            top_resize_border: true,
        }
    }
}

unsafe impl Send for WindowAttributesWindows {}
unsafe impl Sync for WindowAttributesWindows {}

impl WindowAttributesWindows {
    /// Set an owner to the window to be created. Can be used to create a dialog box, for example.
    /// This only works when [`WindowAttributes::with_parent_window`] isn't called or set to `None`.
    /// Can be used in combination with
    /// [`WindowExtWindows::set_enable(false)`][WindowExtWindows::set_enable] on the owner
    /// window to create a modal dialog box.
    ///
    /// From MSDN:
    /// - An owned window is always above its owner in the z-order.
    /// - The system automatically destroys an owned window when its owner is destroyed.
    /// - An owned window is hidden when its owner is minimized.
    ///
    /// For more information, see <https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#owned-windows>
    ///
    /// [`WindowAttributes::with_parent_window`]: crate::window::WindowAttributes::with_parent_window
    pub fn with_owner_window(mut self, parent: HWND) -> Self {
        self.owner = Some(parent);
        self
    }

    /// Sets a menu on the window to be created.
    ///
    /// Parent and menu are mutually exclusive; a child window cannot have a menu!
    ///
    /// The menu must have been manually created beforehand with [`CreateMenu`] or similar.
    ///
    /// Note: Dark mode cannot be supported for win32 menus, it's simply not possible to change how
    /// the menus look. If you use this, it is recommended that you combine it with
    /// `with_theme(Some(Theme::Light))` to avoid a jarring effect.
    #[rustfmt::skip]
    ///
    #[cfg_attr(
        windows_platform,
        doc = "[`CreateMenu`]: windows_sys::Win32::UI::WindowsAndMessaging::CreateMenu"
    )]
    #[cfg_attr(not(windows_platform), doc = "[`CreateMenu`]: #only-available-on-windows")]
    pub fn with_menu(mut self, menu: HMENU) -> Self {
        self.menu = Some(menu);
        self
    }

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    pub fn with_taskbar_icon(mut self, taskbar_icon: Option<Icon>) -> Self {
        self.taskbar_icon = taskbar_icon;
        self
    }

    /// This sets `WS_EX_NOREDIRECTIONBITMAP`.
    pub fn with_no_redirection_bitmap(mut self, flag: bool) -> Self {
        self.no_redirection_bitmap = flag;
        self
    }

    /// Enables/disables the window titlebar by setting `WS_CAPTION`.
    pub fn with_titlebar(mut self, titlebar: bool) -> Self {
        self.titlebar = titlebar;
        self
    }

    /// Enables/disables the window's top resize border by setting its height to 0.
    /// Only for windows without a title bar.
    pub fn with_top_resize_border(mut self, top_resize_border: bool) -> Self {
        self.top_resize_border = top_resize_border;
        self
    }

    /// Enables or disables drag and drop support (enabled by default). Will interfere with other
    /// crates that use multi-threaded COM API (`CoInitializeEx` with `COINIT_MULTITHREADED`
    /// instead of `COINIT_APARTMENTTHREADED`) on the same thread. Note that winit may still
    /// attempt to initialize COM API regardless of this option. Currently only fullscreen mode
    /// does that, but there may be more in the future. If you need COM API with
    /// `COINIT_MULTITHREADED` you must initialize it before calling any winit functions. See <https://docs.microsoft.com/en-us/windows/win32/api/objbase/nf-objbase-coinitialize#remarks> for more information.
    pub fn with_drag_and_drop(mut self, flag: bool) -> Self {
        self.drag_and_drop = flag;
        self
    }

    /// Whether show or hide the window icon in the taskbar.
    pub fn with_skip_taskbar(mut self, skip: bool) -> Self {
        self.skip_taskbar = skip;
        self
    }

    /// Customize the window class name.
    pub fn with_class_name<S: Into<String>>(mut self, class_name: S) -> Self {
        self.class_name = class_name.into();
        self
    }

    /// Shows or hides the background drop shadow for undecorated windows.
    ///
    /// The shadow is hidden by default.
    /// Enabling the shadow causes a thin 1px line to appear on the top of the window.
    pub fn with_undecorated_shadow(mut self, shadow: bool) -> Self {
        self.decoration_shadow = shadow;
        self
    }

    /// Sets system-drawn backdrop type.
    ///
    /// Requires Windows 11 build 22523+.
    pub fn with_system_backdrop(mut self, backdrop_type: BackdropType) -> Self {
        self.backdrop_type = backdrop_type;
        self
    }

    /// This sets or removes `WS_CLIPCHILDREN` style.
    pub fn with_clip_children(mut self, flag: bool) -> Self {
        self.clip_children = flag;
        self
    }

    /// Sets the color of the window border.
    ///
    /// Supported starting with Windows 11 Build 22000.
    pub fn with_border_color(mut self, color: Option<Color>) -> Self {
        self.border_color = Some(color.unwrap_or(Color::NONE));
        self
    }

    /// Sets the background color of the title bar.
    ///
    /// Supported starting with Windows 11 Build 22000.
    pub fn with_title_background_color(mut self, color: Option<Color>) -> Self {
        self.title_background_color = Some(color.unwrap_or(Color::NONE));
        self
    }

    /// Sets the color of the window title.
    ///
    /// Supported starting with Windows 11 Build 22000.
    pub fn with_title_text_color(mut self, color: Color) -> Self {
        self.title_text_color = Some(color);
        self
    }

    /// Sets the preferred style of the window corners.
    ///
    /// Supported starting with Windows 11 Build 22000.
    pub fn with_corner_preference(mut self, corners: CornerPreference) -> Self {
        self.corner_preference = Some(corners);
        self
    }
}

impl PlatformWindowAttributes for WindowAttributesWindows {
    fn box_clone(&self) -> Box<dyn PlatformWindowAttributes> {
        Box::from(self.clone())
    }
}

/// Additional methods on `DeviceId` that are specific to Windows.
pub trait DeviceIdExtWindows {
    /// Returns an identifier that persistently refers to this specific device.
    ///
    /// Will return `None` if the device is no longer available.
    fn persistent_identifier(&self) -> Option<String>;
}

#[cfg(windows_platform)]
impl DeviceIdExtWindows for DeviceId {
    fn persistent_identifier(&self) -> Option<String> {
        let raw_id = self.into_raw();
        if raw_id != 0 {
            crate::platform_impl::raw_input::get_raw_input_device_name(raw_id as HANDLE)
        } else {
            None
        }
    }
}

/// Windows specific `Icon`.
///
/// Windows icons can be created from files, or from the [`embedded resources`](https://learn.microsoft.com/en-us/windows/win32/menurc/about-resource-files).
///
/// The `ICON` resource definition statement use the following syntax:
/// ```rc
/// nameID ICON filename
/// ```
/// `nameID` is a unique name or a 16-bit unsigned integer value identifying the resource,
/// `filename` is the name of the file that contains the resource.
///
/// More information about the `ICON` resource can be found at [`Microsoft Learn`](https://learn.microsoft.com/en-us/windows/win32/menurc/icon-resource) portal.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WinIcon {
    pub(crate) inner: Arc<RaiiIcon>,
}

impl WinIcon {
    /// Create an icon from a file path.
    ///
    /// Specify `size` to load a specific icon size from the file, or `None` to load the default
    /// icon size from the file.
    ///
    /// In cases where the specified size does not exist in the file, Windows may perform scaling
    /// to get an icon of the desired size.
    pub fn from_path<P: AsRef<Path>>(
        path: P,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        Self::from_path_impl(path, size)
    }

    /// Create an icon from a resource embedded in this executable or library by its ordinal id.
    ///
    /// The valid `ordinal` values range from 1 to [`u16::MAX`] (inclusive). The value `0` is an
    /// invalid ordinal id, but it can be used with [`from_resource_name`] as `"0"`.
    ///
    /// [`from_resource_name`]: Self::from_resource_name
    ///
    /// Specify `size` to load a specific icon size from the file, or `None` to load the default
    /// icon size from the file.
    ///
    /// In cases where the specified size does not exist in the file, Windows may perform scaling
    /// to get an icon of the desired size.
    pub fn from_resource(
        resource_id: u16,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        Self::from_resource_impl(resource_id, size)
    }

    /// Create an icon from a resource embedded in this executable or library by its name.
    ///
    /// Specify `size` to load a specific icon size from the file, or `None` to load the default
    /// icon size from the file.
    ///
    /// In cases where the specified size does not exist in the file, Windows may perform scaling
    /// to get an icon of the desired size.
    ///
    /// # Notes
    ///
    /// Consider the following resource definition statements:
    /// ```rc
    /// app     ICON "app.ico"
    /// 1       ICON "a.ico"
    /// 0027    ICON "custom.ico"
    /// 0       ICON "alt.ico"
    /// ```
    ///
    /// Due to some internal implementation details of the resource embedding/loading process on
    /// Windows platform, strings that can be interpreted as 16-bit unsigned integers (`"1"`,
    /// `"002"`, etc.) cannot be used as valid resource names, and instead should be passed into
    /// [`from_resource`]:
    ///
    /// [`from_resource`]: Self::from_resource
    ///
    /// ```rust,no_run
    /// use winit::platform::windows::WinIcon;
    ///
    /// assert!(WinIcon::from_resource_name("app", None).is_ok());
    /// assert!(WinIcon::from_resource(1, None).is_ok());
    /// assert!(WinIcon::from_resource(27, None).is_ok());
    /// assert!(WinIcon::from_resource_name("27", None).is_err());
    /// assert!(WinIcon::from_resource_name("0027", None).is_err());
    /// ```
    ///
    /// While `0` cannot be used as an ordinal id (see [`from_resource`]), it can be used as a
    /// name:
    ///
    /// [`from_resource`]: IconExtWindows::from_resource
    ///
    /// ```rust,no_run
    /// # use winit::platform::windows::WinIcon;
    /// # use winit::icon::Icon;
    /// assert!(WinIcon::from_resource_name("0", None).is_ok());
    /// assert!(WinIcon::from_resource(0, None).is_err());
    /// ```
    pub fn from_resource_name(
        resource_name: &str,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        Self::from_resource_name_impl(resource_name, size)
    }
}

impl From<WinIcon> for Icon {
    fn from(value: WinIcon) -> Self {
        Self(Arc::new(value))
    }
}
