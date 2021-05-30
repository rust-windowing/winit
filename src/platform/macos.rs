#![cfg(target_os = "macos")]

use std::os::raw::c_void;

use crate::{
    dpi::LogicalSize,
    keyboard::{KeyCode, NativeKeyCode},
    monitor::MonitorHandle,
    platform::scancode::KeyCodeExtScancode,
    event_loop::{EventLoop, EventLoopWindowTarget},
    platform_impl::get_aux_state_mut,
    window::{Window, WindowBuilder},
};

/// Additional methods on `Window` that are specific to MacOS.
pub trait WindowExtMacOS {
    /// Returns a pointer to the cocoa `NSWindow` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn ns_window(&self) -> *mut c_void;

    /// Returns a pointer to the cocoa `NSView` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn ns_view(&self) -> *mut c_void;

    /// Returns whether or not the window is in simple fullscreen mode.
    fn simple_fullscreen(&self) -> bool;

    /// Toggles a fullscreen mode that doesn't require a new macOS space.
    /// Returns a boolean indicating whether the transition was successful (this
    /// won't work if the window was already in the native fullscreen).
    ///
    /// This is how fullscreen used to work on macOS in versions before Lion.
    /// And allows the user to have a fullscreen window without using another
    /// space or taking control over the entire monitor.
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool;

    /// Returns whether or not the window has shadow.
    fn has_shadow(&self) -> bool;

    /// Sets whether or not the window has shadow.
    fn set_has_shadow(&self, has_shadow: bool);
}

impl WindowExtMacOS for Window {
    #[inline]
    fn ns_window(&self) -> *mut c_void {
        self.window.ns_window()
    }

    #[inline]
    fn ns_view(&self) -> *mut c_void {
        self.window.ns_view()
    }

    #[inline]
    fn simple_fullscreen(&self) -> bool {
        self.window.simple_fullscreen()
    }

    #[inline]
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool {
        self.window.set_simple_fullscreen(fullscreen)
    }

    #[inline]
    fn has_shadow(&self) -> bool {
        self.window.has_shadow()
    }

    #[inline]
    fn set_has_shadow(&self, has_shadow: bool) {
        self.window.set_has_shadow(has_shadow)
    }
}

/// Corresponds to `NSApplicationActivationPolicy`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivationPolicy {
    /// Corresponds to `NSApplicationActivationPolicyRegular`.
    Regular,
    /// Corresponds to `NSApplicationActivationPolicyAccessory`.
    Accessory,
    /// Corresponds to `NSApplicationActivationPolicyProhibited`.
    Prohibited,
}

impl Default for ActivationPolicy {
    fn default() -> Self {
        ActivationPolicy::Regular
    }
}

/// Additional methods on `WindowBuilder` that are specific to MacOS.
///
/// **Note:** Properties dealing with the titlebar will be overwritten by the `with_decorations` method
/// on the base `WindowBuilder`:
///
///  - `with_titlebar_transparent`
///  - `with_title_hidden`
///  - `with_titlebar_hidden`
///  - `with_titlebar_buttons_hidden`
///  - `with_fullsize_content_view`
pub trait WindowBuilderExtMacOS {
    /// Enables click-and-drag behavior for the entire window, not just the titlebar.
    fn with_movable_by_window_background(self, movable_by_window_background: bool)
        -> WindowBuilder;
    /// Makes the titlebar transparent and allows the content to appear behind it.
    fn with_titlebar_transparent(self, titlebar_transparent: bool) -> WindowBuilder;
    /// Hides the window title.
    fn with_title_hidden(self, title_hidden: bool) -> WindowBuilder;
    /// Hides the window titlebar.
    fn with_titlebar_hidden(self, titlebar_hidden: bool) -> WindowBuilder;
    /// Hides the window titlebar buttons.
    fn with_titlebar_buttons_hidden(self, titlebar_buttons_hidden: bool) -> WindowBuilder;
    /// Makes the window content appear behind the titlebar.
    fn with_fullsize_content_view(self, fullsize_content_view: bool) -> WindowBuilder;
    /// Build window with `resizeIncrements` property. Values must not be 0.
    fn with_resize_increments(self, increments: LogicalSize<f64>) -> WindowBuilder;
    fn with_disallow_hidpi(self, disallow_hidpi: bool) -> WindowBuilder;
    fn with_has_shadow(self, has_shadow: bool) -> WindowBuilder;
}

impl WindowBuilderExtMacOS for WindowBuilder {
    #[inline]
    fn with_movable_by_window_background(
        mut self,
        movable_by_window_background: bool,
    ) -> WindowBuilder {
        self.platform_specific.movable_by_window_background = movable_by_window_background;
        self
    }

    #[inline]
    fn with_titlebar_transparent(mut self, titlebar_transparent: bool) -> WindowBuilder {
        self.platform_specific.titlebar_transparent = titlebar_transparent;
        self
    }

    #[inline]
    fn with_titlebar_hidden(mut self, titlebar_hidden: bool) -> WindowBuilder {
        self.platform_specific.titlebar_hidden = titlebar_hidden;
        self
    }

    #[inline]
    fn with_titlebar_buttons_hidden(mut self, titlebar_buttons_hidden: bool) -> WindowBuilder {
        self.platform_specific.titlebar_buttons_hidden = titlebar_buttons_hidden;
        self
    }

    #[inline]
    fn with_title_hidden(mut self, title_hidden: bool) -> WindowBuilder {
        self.platform_specific.title_hidden = title_hidden;
        self
    }

    #[inline]
    fn with_fullsize_content_view(mut self, fullsize_content_view: bool) -> WindowBuilder {
        self.platform_specific.fullsize_content_view = fullsize_content_view;
        self
    }

    #[inline]
    fn with_resize_increments(mut self, increments: LogicalSize<f64>) -> WindowBuilder {
        self.platform_specific.resize_increments = Some(increments.into());
        self
    }

    #[inline]
    fn with_disallow_hidpi(mut self, disallow_hidpi: bool) -> WindowBuilder {
        self.platform_specific.disallow_hidpi = disallow_hidpi;
        self
    }

    #[inline]
    fn with_has_shadow(mut self, has_shadow: bool) -> WindowBuilder {
        self.platform_specific.has_shadow = has_shadow;
        self
    }
}

pub trait EventLoopExtMacOS {
    /// Sets the activation policy for the application. It is set to
    /// `NSApplicationActivationPolicyRegular` by default.
    ///
    /// This function only takes effect if it's called before calling [`run`](crate::event_loop::EventLoop::run) or
    /// [`run_return`](crate::platform::run_return::EventLoopExtRunReturn::run_return)
    fn set_activation_policy(&mut self, activation_policy: ActivationPolicy);

    /// Used to prevent a default menubar menu from getting created
    ///
    /// The default menu creation is enabled by default.
    ///
    /// This function only takes effect if it's called before calling
    /// [`run`](crate::event_loop::EventLoop::run) or
    /// [`run_return`](crate::platform::run_return::EventLoopExtRunReturn::run_return)
    fn enable_default_menu_creation(&mut self, enable: bool);
}
impl<T> EventLoopExtMacOS for EventLoop<T> {
    #[inline]
    fn set_activation_policy(&mut self, activation_policy: ActivationPolicy) {
        unsafe {
            get_aux_state_mut(&**self.event_loop.delegate).activation_policy = activation_policy;
        }
    }

    #[inline]
    fn enable_default_menu_creation(&mut self, enable: bool) {
        unsafe {
            get_aux_state_mut(&**self.event_loop.delegate).create_default_menu = enable;
        }
    }
}

/// Additional methods on `MonitorHandle` that are specific to MacOS.
pub trait MonitorHandleExtMacOS {
    /// Returns the identifier of the monitor for Cocoa.
    fn native_id(&self) -> u32;
    /// Returns a pointer to the NSScreen representing this monitor.
    fn ns_screen(&self) -> Option<*mut c_void>;
}

impl MonitorHandleExtMacOS for MonitorHandle {
    #[inline]
    fn native_id(&self) -> u32 {
        self.inner.native_identifier()
    }

    fn ns_screen(&self) -> Option<*mut c_void> {
        self.inner.ns_screen().map(|s| s as *mut c_void)
    }
}

/// Additional methods on `EventLoopWindowTarget` that are specific to macOS.
pub trait EventLoopWindowTargetExtMacOS {
    /// Hide the entire application. In most applications this is typically triggered with Command-H.
    fn hide_application(&self);
    /// Hide the other applications. In most applications this is typically triggered with Command+Option-H.
    fn hide_other_applications(&self);
}

impl<T> EventLoopWindowTargetExtMacOS for EventLoopWindowTarget<T> {
    fn hide_application(&self) {
        let cls = objc::runtime::Class::get("NSApplication").unwrap();
        let app: cocoa::base::id = unsafe { msg_send![cls, sharedApplication] };
        unsafe { msg_send![app, hide: 0] }
    }

    fn hide_other_applications(&self) {
        let cls = objc::runtime::Class::get("NSApplication").unwrap();
        let app: cocoa::base::id = unsafe { msg_send![cls, sharedApplication] };
        unsafe { msg_send![app, hideOtherApplications: 0] }
    }
}

impl KeyCodeExtScancode for KeyCode {
    fn to_scancode(self) -> Option<u32> {
        match self {
            KeyCode::KeyA => Some(0x00),
            KeyCode::KeyS => Some(0x01),
            KeyCode::KeyD => Some(0x02),
            KeyCode::KeyF => Some(0x03),
            KeyCode::KeyH => Some(0x04),
            KeyCode::KeyG => Some(0x05),
            KeyCode::KeyZ => Some(0x06),
            KeyCode::KeyX => Some(0x07),
            KeyCode::KeyC => Some(0x08),
            KeyCode::KeyV => Some(0x09),
            KeyCode::KeyB => Some(0x0b),
            KeyCode::KeyQ => Some(0x0c),
            KeyCode::KeyW => Some(0x0d),
            KeyCode::KeyE => Some(0x0e),
            KeyCode::KeyR => Some(0x0f),
            KeyCode::KeyY => Some(0x10),
            KeyCode::KeyT => Some(0x11),
            KeyCode::Digit1 => Some(0x12),
            KeyCode::Digit2 => Some(0x13),
            KeyCode::Digit3 => Some(0x14),
            KeyCode::Digit4 => Some(0x15),
            KeyCode::Digit6 => Some(0x16),
            KeyCode::Digit5 => Some(0x17),
            KeyCode::Equal => Some(0x18),
            KeyCode::Digit9 => Some(0x19),
            KeyCode::Digit7 => Some(0x1a),
            KeyCode::Minus => Some(0x1b),
            KeyCode::Digit8 => Some(0x1c),
            KeyCode::Digit0 => Some(0x1d),
            KeyCode::BracketRight => Some(0x1e),
            KeyCode::KeyO => Some(0x1f),
            KeyCode::KeyU => Some(0x20),
            KeyCode::BracketLeft => Some(0x21),
            KeyCode::KeyI => Some(0x22),
            KeyCode::KeyP => Some(0x23),
            KeyCode::Enter => Some(0x24),
            KeyCode::KeyL => Some(0x25),
            KeyCode::KeyJ => Some(0x26),
            KeyCode::Quote => Some(0x27),
            KeyCode::KeyK => Some(0x28),
            KeyCode::Semicolon => Some(0x29),
            KeyCode::Backslash => Some(0x2a),
            KeyCode::Comma => Some(0x2b),
            KeyCode::Slash => Some(0x2c),
            KeyCode::KeyN => Some(0x2d),
            KeyCode::KeyM => Some(0x2e),
            KeyCode::Period => Some(0x2f),
            KeyCode::Tab => Some(0x30),
            KeyCode::Space => Some(0x31),
            KeyCode::Backquote => Some(0x32),
            KeyCode::Backspace => Some(0x33),
            KeyCode::Escape => Some(0x35),
            KeyCode::SuperRight => Some(0x36),
            KeyCode::SuperLeft => Some(0x37),
            KeyCode::ShiftLeft => Some(0x38),
            KeyCode::AltLeft => Some(0x3a),
            KeyCode::ControlLeft => Some(0x3b),
            KeyCode::ShiftRight => Some(0x3c),
            KeyCode::AltRight => Some(0x3d),
            KeyCode::ControlRight => Some(0x3e),
            KeyCode::F17 => Some(0x40),
            KeyCode::NumpadDecimal => Some(0x41),
            KeyCode::NumpadMultiply => Some(0x43),
            KeyCode::NumpadAdd => Some(0x45),
            KeyCode::NumLock => Some(0x47),
            KeyCode::AudioVolumeUp => Some(0x49),
            KeyCode::AudioVolumeDown => Some(0x4a),
            KeyCode::NumpadDivide => Some(0x4b),
            KeyCode::NumpadEnter => Some(0x4c),
            KeyCode::NumpadSubtract => Some(0x4e),
            KeyCode::F18 => Some(0x4f),
            KeyCode::F19 => Some(0x50),
            KeyCode::NumpadEqual => Some(0x51),
            KeyCode::Numpad0 => Some(0x52),
            KeyCode::Numpad1 => Some(0x53),
            KeyCode::Numpad2 => Some(0x54),
            KeyCode::Numpad3 => Some(0x55),
            KeyCode::Numpad4 => Some(0x56),
            KeyCode::Numpad5 => Some(0x57),
            KeyCode::Numpad6 => Some(0x58),
            KeyCode::Numpad7 => Some(0x59),
            KeyCode::F20 => Some(0x5a),
            KeyCode::Numpad8 => Some(0x5b),
            KeyCode::Numpad9 => Some(0x5c),
            KeyCode::IntlYen => Some(0x5d),
            KeyCode::F5 => Some(0x60),
            KeyCode::F6 => Some(0x61),
            KeyCode::F7 => Some(0x62),
            KeyCode::F3 => Some(0x63),
            KeyCode::F8 => Some(0x64),
            KeyCode::F9 => Some(0x65),
            KeyCode::F11 => Some(0x67),
            KeyCode::F13 => Some(0x69),
            KeyCode::F16 => Some(0x6a),
            KeyCode::F14 => Some(0x6b),
            KeyCode::F10 => Some(0x6d),
            KeyCode::F12 => Some(0x6f),
            KeyCode::F15 => Some(0x71),
            KeyCode::Insert => Some(0x72),
            KeyCode::Home => Some(0x73),
            KeyCode::PageUp => Some(0x74),
            KeyCode::Delete => Some(0x75),
            KeyCode::F4 => Some(0x76),
            KeyCode::End => Some(0x77),
            KeyCode::F2 => Some(0x78),
            KeyCode::PageDown => Some(0x79),
            KeyCode::F1 => Some(0x7a),
            KeyCode::ArrowLeft => Some(0x7b),
            KeyCode::ArrowRight => Some(0x7c),
            KeyCode::ArrowDown => Some(0x7d),
            KeyCode::ArrowUp => Some(0x7e),
            _ => None,
        }
    }

    fn from_scancode(scancode: u32) -> KeyCode {
        match scancode {
            0x00 => KeyCode::KeyA,
            0x01 => KeyCode::KeyS,
            0x02 => KeyCode::KeyD,
            0x03 => KeyCode::KeyF,
            0x04 => KeyCode::KeyH,
            0x05 => KeyCode::KeyG,
            0x06 => KeyCode::KeyZ,
            0x07 => KeyCode::KeyX,
            0x08 => KeyCode::KeyC,
            0x09 => KeyCode::KeyV,
            //0x0a => World 1,
            0x0b => KeyCode::KeyB,
            0x0c => KeyCode::KeyQ,
            0x0d => KeyCode::KeyW,
            0x0e => KeyCode::KeyE,
            0x0f => KeyCode::KeyR,
            0x10 => KeyCode::KeyY,
            0x11 => KeyCode::KeyT,
            0x12 => KeyCode::Digit1,
            0x13 => KeyCode::Digit2,
            0x14 => KeyCode::Digit3,
            0x15 => KeyCode::Digit4,
            0x16 => KeyCode::Digit6,
            0x17 => KeyCode::Digit5,
            0x18 => KeyCode::Equal,
            0x19 => KeyCode::Digit9,
            0x1a => KeyCode::Digit7,
            0x1b => KeyCode::Minus,
            0x1c => KeyCode::Digit8,
            0x1d => KeyCode::Digit0,
            0x1e => KeyCode::BracketRight,
            0x1f => KeyCode::KeyO,
            0x20 => KeyCode::KeyU,
            0x21 => KeyCode::BracketLeft,
            0x22 => KeyCode::KeyI,
            0x23 => KeyCode::KeyP,
            0x24 => KeyCode::Enter,
            0x25 => KeyCode::KeyL,
            0x26 => KeyCode::KeyJ,
            0x27 => KeyCode::Quote,
            0x28 => KeyCode::KeyK,
            0x29 => KeyCode::Semicolon,
            0x2a => KeyCode::Backslash,
            0x2b => KeyCode::Comma,
            0x2c => KeyCode::Slash,
            0x2d => KeyCode::KeyN,
            0x2e => KeyCode::KeyM,
            0x2f => KeyCode::Period,
            0x30 => KeyCode::Tab,
            0x31 => KeyCode::Space,
            0x32 => KeyCode::Backquote,
            0x33 => KeyCode::Backspace,
            //0x34 => unknown,
            0x35 => KeyCode::Escape,
            0x36 => KeyCode::SuperRight,
            0x37 => KeyCode::SuperLeft,
            0x38 => KeyCode::ShiftLeft,
            0x39 => KeyCode::CapsLock,
            0x3a => KeyCode::AltLeft,
            0x3b => KeyCode::ControlLeft,
            0x3c => KeyCode::ShiftRight,
            0x3d => KeyCode::AltRight,
            0x3e => KeyCode::ControlRight,
            0x3f => KeyCode::Fn,
            0x40 => KeyCode::F17,
            0x41 => KeyCode::NumpadDecimal,
            //0x42 -> unknown,
            0x43 => KeyCode::NumpadMultiply,
            //0x44 => unknown,
            0x45 => KeyCode::NumpadAdd,
            //0x46 => unknown,
            0x47 => KeyCode::NumLock,
            //0x48 => KeyCode::NumpadClear,

            // TODO: (Artur) for me, kVK_VolumeUp is 0x48
            // macOS 10.11
            // /System/Library/Frameworks/Carbon.framework/Versions/A/Frameworks/HIToolbox.framework/Versions/A/Headers/Events.h
            0x49 => KeyCode::AudioVolumeUp,
            0x4a => KeyCode::AudioVolumeDown,
            0x4b => KeyCode::NumpadDivide,
            0x4c => KeyCode::NumpadEnter,
            //0x4d => unknown,
            0x4e => KeyCode::NumpadSubtract,
            0x4f => KeyCode::F18,
            0x50 => KeyCode::F19,
            0x51 => KeyCode::NumpadEqual,
            0x52 => KeyCode::Numpad0,
            0x53 => KeyCode::Numpad1,
            0x54 => KeyCode::Numpad2,
            0x55 => KeyCode::Numpad3,
            0x56 => KeyCode::Numpad4,
            0x57 => KeyCode::Numpad5,
            0x58 => KeyCode::Numpad6,
            0x59 => KeyCode::Numpad7,
            0x5a => KeyCode::F20,
            0x5b => KeyCode::Numpad8,
            0x5c => KeyCode::Numpad9,
            0x5d => KeyCode::IntlYen,
            //0x5e => JIS Ro,
            //0x5f => unknown,
            0x60 => KeyCode::F5,
            0x61 => KeyCode::F6,
            0x62 => KeyCode::F7,
            0x63 => KeyCode::F3,
            0x64 => KeyCode::F8,
            0x65 => KeyCode::F9,
            //0x66 => JIS Eisuu (macOS),
            0x67 => KeyCode::F11,
            //0x68 => JIS Kanna (macOS),
            0x69 => KeyCode::F13,
            0x6a => KeyCode::F16,
            0x6b => KeyCode::F14,
            //0x6c => unknown,
            0x6d => KeyCode::F10,
            //0x6e => unknown,
            0x6f => KeyCode::F12,
            //0x70 => unknown,
            0x71 => KeyCode::F15,
            0x72 => KeyCode::Insert,
            0x73 => KeyCode::Home,
            0x74 => KeyCode::PageUp,
            0x75 => KeyCode::Delete,
            0x76 => KeyCode::F4,
            0x77 => KeyCode::End,
            0x78 => KeyCode::F2,
            0x79 => KeyCode::PageDown,
            0x7a => KeyCode::F1,
            0x7b => KeyCode::ArrowLeft,
            0x7c => KeyCode::ArrowRight,
            0x7d => KeyCode::ArrowDown,
            0x7e => KeyCode::ArrowUp,
            //0x7f =>  unknown,

            // 0xA is the caret (^) an macOS's German QERTZ layout. This key is at the same location as
            // backquote (`) on Windows' US layout.
            0xa => KeyCode::Backquote,
            _ => KeyCode::Unidentified(NativeKeyCode::MacOS(scancode as u16)),
        }
    }
}
