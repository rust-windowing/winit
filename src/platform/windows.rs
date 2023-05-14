use std::{ffi::c_void, path::Path};

use windows_sys::Win32::{
    System::SystemServices::LANG_KOREAN, UI::Input::KeyboardAndMouse::GetKeyboardLayout,
};

use crate::{
    dpi::PhysicalSize,
    event::{DeviceId, KeyEvent},
    event_loop::EventLoopBuilder,
    keyboard::{Key, KeyCode, NativeKeyCode},
    monitor::MonitorHandle,
    platform::{modifier_supplement::KeyEventExtModifierSupplement, scancode::KeyCodeExtScancode},
    platform_impl::{loword, primarylangid, WinIcon},
    window::{BadIcon, Icon, Window, WindowBuilder},
};

/// Window Handle type used by Win32 API
pub type HWND = isize;
/// Menu Handle type used by Win32 API
pub type HMENU = isize;
/// Monitor Handle type used by Win32 API
pub type HMONITOR = isize;
/// Instance Handle type used by Win32 API
pub type HINSTANCE = isize;

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
    /// Returns the HINSTANCE of the window
    fn hinstance(&self) -> HINSTANCE;
    /// Returns the native handle that is used by this window.
    ///
    /// The pointer will become invalid when the native window was destroyed.
    fn hwnd(&self) -> HWND;

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
}

impl WindowExtWindows for Window {
    #[inline]
    fn hinstance(&self) -> HINSTANCE {
        self.window.hinstance()
    }

    #[inline]
    fn hwnd(&self) -> HWND {
        self.window.hwnd()
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
    fn set_skip_taskbar(&self, skip: bool) {
        self.window.set_skip_taskbar(skip)
    }

    #[inline]
    fn set_undecorated_shadow(&self, shadow: bool) {
        self.window.set_undecorated_shadow(shadow)
    }
}

/// Additional methods on `WindowBuilder` that are specific to Windows.
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
    fn with_owner_window(self, parent: HWND) -> WindowBuilder;

    /// Sets a menu on the window to be created.
    ///
    /// Parent and menu are mutually exclusive; a child window cannot have a menu!
    ///
    /// The menu must have been manually created beforehand with [`CreateMenu`] or similar.
    ///
    /// Note: Dark mode cannot be supported for win32 menus, it's simply not possible to change how the menus look.
    /// If you use this, it is recommended that you combine it with `with_theme(Some(Theme::Light))` to avoid a jarring effect.
    ///
    /// [`CreateMenu`]: windows_sys::Win32::UI::WindowsAndMessaging::CreateMenu
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

    /// Whether show or hide the window icon in the taskbar.
    fn with_skip_taskbar(self, skip: bool) -> WindowBuilder;

    /// Shows or hides the background drop shadow for undecorated windows.
    ///
    /// The shadow is hidden by default.
    /// Enabling the shadow causes a thin 1px line to appear on the top of the window.
    fn with_undecorated_shadow(self, shadow: bool) -> WindowBuilder;
}

impl WindowBuilderExtWindows for WindowBuilder {
    #[inline]
    fn with_owner_window(mut self, parent: HWND) -> WindowBuilder {
        self.platform_specific.owner = Some(parent);
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
    fn with_skip_taskbar(mut self, skip: bool) -> WindowBuilder {
        self.platform_specific.skip_taskbar = skip;
        self
    }

    #[inline]
    fn with_undecorated_shadow(mut self, shadow: bool) -> WindowBuilder {
        self.platform_specific.decoration_shadow = shadow;
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
        let win_icon = WinIcon::from_path(path, size)?;
        Ok(Icon { inner: win_icon })
    }

    fn from_resource(ordinal: u16, size: Option<PhysicalSize<u32>>) -> Result<Self, BadIcon> {
        let win_icon = WinIcon::from_resource(ordinal, size)?;
        Ok(Icon { inner: win_icon })
    }
}

impl KeyEventExtModifierSupplement for KeyEvent {
    #[inline]
    fn text_with_all_modifiers(&self) -> Option<&str> {
        self.platform_specific.text_with_all_modifers
    }

    #[inline]
    fn key_without_modifiers(&self) -> Key<'static> {
        self.platform_specific.key_without_modifiers.clone()
    }
}

impl KeyCodeExtScancode for KeyCode {
    fn to_scancode(self) -> Option<u32> {
        // See `from_scancode` for more info

        let hkl = unsafe { GetKeyboardLayout(0) };

        let primary_lang_id = primarylangid(loword(hkl as u32));
        let is_korean = primary_lang_id as u32 == LANG_KOREAN;

        match self {
            KeyCode::Backquote => Some(0x0029),
            KeyCode::Backslash => Some(0x002B),
            KeyCode::Backspace => Some(0x000E),
            KeyCode::BracketLeft => Some(0x001A),
            KeyCode::BracketRight => Some(0x001B),
            KeyCode::Comma => Some(0x0033),
            KeyCode::Digit0 => Some(0x000B),
            KeyCode::Digit1 => Some(0x0002),
            KeyCode::Digit2 => Some(0x0003),
            KeyCode::Digit3 => Some(0x0004),
            KeyCode::Digit4 => Some(0x0005),
            KeyCode::Digit5 => Some(0x0006),
            KeyCode::Digit6 => Some(0x0007),
            KeyCode::Digit7 => Some(0x0008),
            KeyCode::Digit8 => Some(0x0009),
            KeyCode::Digit9 => Some(0x000A),
            KeyCode::Equal => Some(0x000D),
            KeyCode::IntlBackslash => Some(0x0056),
            KeyCode::IntlRo => Some(0x0073),
            KeyCode::IntlYen => Some(0x007D),
            KeyCode::KeyA => Some(0x001E),
            KeyCode::KeyB => Some(0x0030),
            KeyCode::KeyC => Some(0x002E),
            KeyCode::KeyD => Some(0x0020),
            KeyCode::KeyE => Some(0x0012),
            KeyCode::KeyF => Some(0x0021),
            KeyCode::KeyG => Some(0x0022),
            KeyCode::KeyH => Some(0x0023),
            KeyCode::KeyI => Some(0x0017),
            KeyCode::KeyJ => Some(0x0024),
            KeyCode::KeyK => Some(0x0025),
            KeyCode::KeyL => Some(0x0026),
            KeyCode::KeyM => Some(0x0032),
            KeyCode::KeyN => Some(0x0031),
            KeyCode::KeyO => Some(0x0018),
            KeyCode::KeyP => Some(0x0019),
            KeyCode::KeyQ => Some(0x0010),
            KeyCode::KeyR => Some(0x0013),
            KeyCode::KeyS => Some(0x001F),
            KeyCode::KeyT => Some(0x0014),
            KeyCode::KeyU => Some(0x0016),
            KeyCode::KeyV => Some(0x002F),
            KeyCode::KeyW => Some(0x0011),
            KeyCode::KeyX => Some(0x002D),
            KeyCode::KeyY => Some(0x0015),
            KeyCode::KeyZ => Some(0x002C),
            KeyCode::Minus => Some(0x000C),
            KeyCode::Period => Some(0x0034),
            KeyCode::Quote => Some(0x0028),
            KeyCode::Semicolon => Some(0x0027),
            KeyCode::Slash => Some(0x0035),
            KeyCode::AltLeft => Some(0x0038),
            KeyCode::AltRight => Some(0xE038),
            KeyCode::CapsLock => Some(0x003A),
            KeyCode::ContextMenu => Some(0xE05D),
            KeyCode::ControlLeft => Some(0x001D),
            KeyCode::ControlRight => Some(0xE01D),
            KeyCode::Enter => Some(0x001C),
            KeyCode::SuperLeft => Some(0xE05B),
            KeyCode::SuperRight => Some(0xE05C),
            KeyCode::ShiftLeft => Some(0x002A),
            KeyCode::ShiftRight => Some(0x0036),
            KeyCode::Space => Some(0x0039),
            KeyCode::Tab => Some(0x000F),
            KeyCode::Convert => Some(0x0079),
            KeyCode::Lang1 => {
                if is_korean {
                    Some(0xE0F2)
                } else {
                    Some(0x0072)
                }
            }
            KeyCode::Lang2 => {
                if is_korean {
                    Some(0xE0F1)
                } else {
                    Some(0x0071)
                }
            }
            KeyCode::KanaMode => Some(0x0070),
            KeyCode::NonConvert => Some(0x007B),
            KeyCode::Delete => Some(0xE053),
            KeyCode::End => Some(0xE04F),
            KeyCode::Home => Some(0xE047),
            KeyCode::Insert => Some(0xE052),
            KeyCode::PageDown => Some(0xE051),
            KeyCode::PageUp => Some(0xE049),
            KeyCode::ArrowDown => Some(0xE050),
            KeyCode::ArrowLeft => Some(0xE04B),
            KeyCode::ArrowRight => Some(0xE04D),
            KeyCode::ArrowUp => Some(0xE048),
            KeyCode::NumLock => Some(0xE045),
            KeyCode::Numpad0 => Some(0x0052),
            KeyCode::Numpad1 => Some(0x004F),
            KeyCode::Numpad2 => Some(0x0050),
            KeyCode::Numpad3 => Some(0x0051),
            KeyCode::Numpad4 => Some(0x004B),
            KeyCode::Numpad5 => Some(0x004C),
            KeyCode::Numpad6 => Some(0x004D),
            KeyCode::Numpad7 => Some(0x0047),
            KeyCode::Numpad8 => Some(0x0048),
            KeyCode::Numpad9 => Some(0x0049),
            KeyCode::NumpadAdd => Some(0x004E),
            KeyCode::NumpadComma => Some(0x007E),
            KeyCode::NumpadDecimal => Some(0x0053),
            KeyCode::NumpadDivide => Some(0xE035),
            KeyCode::NumpadEnter => Some(0xE01C),
            KeyCode::NumpadEqual => Some(0x0059),
            KeyCode::NumpadMultiply => Some(0x0037),
            KeyCode::NumpadSubtract => Some(0x004A),
            KeyCode::Escape => Some(0x0001),
            KeyCode::F1 => Some(0x003B),
            KeyCode::F2 => Some(0x003C),
            KeyCode::F3 => Some(0x003D),
            KeyCode::F4 => Some(0x003E),
            KeyCode::F5 => Some(0x003F),
            KeyCode::F6 => Some(0x0040),
            KeyCode::F7 => Some(0x0041),
            KeyCode::F8 => Some(0x0042),
            KeyCode::F9 => Some(0x0043),
            KeyCode::F10 => Some(0x0044),
            KeyCode::F11 => Some(0x0057),
            KeyCode::F12 => Some(0x0058),
            KeyCode::F13 => Some(0x0064),
            KeyCode::F14 => Some(0x0065),
            KeyCode::F15 => Some(0x0066),
            KeyCode::F16 => Some(0x0067),
            KeyCode::F17 => Some(0x0068),
            KeyCode::F18 => Some(0x0069),
            KeyCode::F19 => Some(0x006A),
            KeyCode::F20 => Some(0x006B),
            KeyCode::F21 => Some(0x006C),
            KeyCode::F22 => Some(0x006D),
            KeyCode::F23 => Some(0x006E),
            KeyCode::F24 => Some(0x0076),
            KeyCode::PrintScreen => Some(0xE037),
            //KeyCode::PrintScreen => Some(0x0054), // Alt + PrintScreen
            KeyCode::ScrollLock => Some(0x0046),
            KeyCode::Pause => Some(0x0045),
            //KeyCode::Pause => Some(0xE046), // Ctrl + Pause
            KeyCode::BrowserBack => Some(0xE06A),
            KeyCode::BrowserFavorites => Some(0xE066),
            KeyCode::BrowserForward => Some(0xE069),
            KeyCode::BrowserHome => Some(0xE032),
            KeyCode::BrowserRefresh => Some(0xE067),
            KeyCode::BrowserSearch => Some(0xE065),
            KeyCode::BrowserStop => Some(0xE068),
            KeyCode::LaunchApp1 => Some(0xE06B),
            KeyCode::LaunchApp2 => Some(0xE021),
            KeyCode::LaunchMail => Some(0xE06C),
            KeyCode::MediaPlayPause => Some(0xE022),
            KeyCode::MediaSelect => Some(0xE06D),
            KeyCode::MediaStop => Some(0xE024),
            KeyCode::MediaTrackNext => Some(0xE019),
            KeyCode::MediaTrackPrevious => Some(0xE010),
            KeyCode::Power => Some(0xE05E),
            KeyCode::AudioVolumeDown => Some(0xE02E),
            KeyCode::AudioVolumeMute => Some(0xE020),
            KeyCode::AudioVolumeUp => Some(0xE030),
            KeyCode::Unidentified(NativeKeyCode::Windows(scancode)) => Some(scancode as u32),
            _ => None,
        }
    }

    fn from_scancode(scancode: u32) -> KeyCode {
        // See: https://www.win.tue.nl/~aeb/linux/kbd/scancodes-1.html
        // and: https://www.w3.org/TR/uievents-code/
        // and: The widget/NativeKeyToDOMCodeName.h file in the firefox source

        match scancode {
            0x0029 => KeyCode::Backquote,
            0x002B => KeyCode::Backslash,
            0x000E => KeyCode::Backspace,
            0x001A => KeyCode::BracketLeft,
            0x001B => KeyCode::BracketRight,
            0x0033 => KeyCode::Comma,
            0x000B => KeyCode::Digit0,
            0x0002 => KeyCode::Digit1,
            0x0003 => KeyCode::Digit2,
            0x0004 => KeyCode::Digit3,
            0x0005 => KeyCode::Digit4,
            0x0006 => KeyCode::Digit5,
            0x0007 => KeyCode::Digit6,
            0x0008 => KeyCode::Digit7,
            0x0009 => KeyCode::Digit8,
            0x000A => KeyCode::Digit9,
            0x000D => KeyCode::Equal,
            0x0056 => KeyCode::IntlBackslash,
            0x0073 => KeyCode::IntlRo,
            0x007D => KeyCode::IntlYen,
            0x001E => KeyCode::KeyA,
            0x0030 => KeyCode::KeyB,
            0x002E => KeyCode::KeyC,
            0x0020 => KeyCode::KeyD,
            0x0012 => KeyCode::KeyE,
            0x0021 => KeyCode::KeyF,
            0x0022 => KeyCode::KeyG,
            0x0023 => KeyCode::KeyH,
            0x0017 => KeyCode::KeyI,
            0x0024 => KeyCode::KeyJ,
            0x0025 => KeyCode::KeyK,
            0x0026 => KeyCode::KeyL,
            0x0032 => KeyCode::KeyM,
            0x0031 => KeyCode::KeyN,
            0x0018 => KeyCode::KeyO,
            0x0019 => KeyCode::KeyP,
            0x0010 => KeyCode::KeyQ,
            0x0013 => KeyCode::KeyR,
            0x001F => KeyCode::KeyS,
            0x0014 => KeyCode::KeyT,
            0x0016 => KeyCode::KeyU,
            0x002F => KeyCode::KeyV,
            0x0011 => KeyCode::KeyW,
            0x002D => KeyCode::KeyX,
            0x0015 => KeyCode::KeyY,
            0x002C => KeyCode::KeyZ,
            0x000C => KeyCode::Minus,
            0x0034 => KeyCode::Period,
            0x0028 => KeyCode::Quote,
            0x0027 => KeyCode::Semicolon,
            0x0035 => KeyCode::Slash,
            0x0038 => KeyCode::AltLeft,
            0xE038 => KeyCode::AltRight,
            0x003A => KeyCode::CapsLock,
            0xE05D => KeyCode::ContextMenu,
            0x001D => KeyCode::ControlLeft,
            0xE01D => KeyCode::ControlRight,
            0x001C => KeyCode::Enter,
            0xE05B => KeyCode::SuperLeft,
            0xE05C => KeyCode::SuperRight,
            0x002A => KeyCode::ShiftLeft,
            0x0036 => KeyCode::ShiftRight,
            0x0039 => KeyCode::Space,
            0x000F => KeyCode::Tab,
            0x0079 => KeyCode::Convert,
            0x0072 => KeyCode::Lang1, // for non-Korean layout
            0xE0F2 => KeyCode::Lang1, // for Korean layout
            0x0071 => KeyCode::Lang2, // for non-Korean layout
            0xE0F1 => KeyCode::Lang2, // for Korean layout
            0x0070 => KeyCode::KanaMode,
            0x007B => KeyCode::NonConvert,
            0xE053 => KeyCode::Delete,
            0xE04F => KeyCode::End,
            0xE047 => KeyCode::Home,
            0xE052 => KeyCode::Insert,
            0xE051 => KeyCode::PageDown,
            0xE049 => KeyCode::PageUp,
            0xE050 => KeyCode::ArrowDown,
            0xE04B => KeyCode::ArrowLeft,
            0xE04D => KeyCode::ArrowRight,
            0xE048 => KeyCode::ArrowUp,
            0xE045 => KeyCode::NumLock,
            0x0052 => KeyCode::Numpad0,
            0x004F => KeyCode::Numpad1,
            0x0050 => KeyCode::Numpad2,
            0x0051 => KeyCode::Numpad3,
            0x004B => KeyCode::Numpad4,
            0x004C => KeyCode::Numpad5,
            0x004D => KeyCode::Numpad6,
            0x0047 => KeyCode::Numpad7,
            0x0048 => KeyCode::Numpad8,
            0x0049 => KeyCode::Numpad9,
            0x004E => KeyCode::NumpadAdd,
            0x007E => KeyCode::NumpadComma,
            0x0053 => KeyCode::NumpadDecimal,
            0xE035 => KeyCode::NumpadDivide,
            0xE01C => KeyCode::NumpadEnter,
            0x0059 => KeyCode::NumpadEqual,
            0x0037 => KeyCode::NumpadMultiply,
            0x004A => KeyCode::NumpadSubtract,
            0x0001 => KeyCode::Escape,
            0x003B => KeyCode::F1,
            0x003C => KeyCode::F2,
            0x003D => KeyCode::F3,
            0x003E => KeyCode::F4,
            0x003F => KeyCode::F5,
            0x0040 => KeyCode::F6,
            0x0041 => KeyCode::F7,
            0x0042 => KeyCode::F8,
            0x0043 => KeyCode::F9,
            0x0044 => KeyCode::F10,
            0x0057 => KeyCode::F11,
            0x0058 => KeyCode::F12,
            0x0064 => KeyCode::F13,
            0x0065 => KeyCode::F14,
            0x0066 => KeyCode::F15,
            0x0067 => KeyCode::F16,
            0x0068 => KeyCode::F17,
            0x0069 => KeyCode::F18,
            0x006A => KeyCode::F19,
            0x006B => KeyCode::F20,
            0x006C => KeyCode::F21,
            0x006D => KeyCode::F22,
            0x006E => KeyCode::F23,
            0x0076 => KeyCode::F24,
            0xE037 => KeyCode::PrintScreen,
            0x0054 => KeyCode::PrintScreen, // Alt + PrintScreen
            0x0046 => KeyCode::ScrollLock,
            0x0045 => KeyCode::Pause,
            0xE046 => KeyCode::Pause, // Ctrl + Pause
            0xE06A => KeyCode::BrowserBack,
            0xE066 => KeyCode::BrowserFavorites,
            0xE069 => KeyCode::BrowserForward,
            0xE032 => KeyCode::BrowserHome,
            0xE067 => KeyCode::BrowserRefresh,
            0xE065 => KeyCode::BrowserSearch,
            0xE068 => KeyCode::BrowserStop,
            0xE06B => KeyCode::LaunchApp1,
            0xE021 => KeyCode::LaunchApp2,
            0xE06C => KeyCode::LaunchMail,
            0xE022 => KeyCode::MediaPlayPause,
            0xE06D => KeyCode::MediaSelect,
            0xE024 => KeyCode::MediaStop,
            0xE019 => KeyCode::MediaTrackNext,
            0xE010 => KeyCode::MediaTrackPrevious,
            0xE05E => KeyCode::Power,
            0xE02E => KeyCode::AudioVolumeDown,
            0xE020 => KeyCode::AudioVolumeMute,
            0xE030 => KeyCode::AudioVolumeUp,
            _ => KeyCode::Unidentified(NativeKeyCode::Windows(scancode as u16)),
        }
    }
}
