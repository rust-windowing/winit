#![cfg(target_os = "macos")]

use std::os::raw::c_void;

use crate::{
    dpi::LogicalSize,
    monitor::MonitorHandle,
    window::{Window, WindowBuilder},
};

/// Corresponds to `NSRequestUserAttentionType`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestUserAttentionType {
    /// Corresponds to `NSCriticalRequest`.
    ///
    /// Dock icon will bounce until the application is focused.
    Critical,

    /// Corresponds to `NSInformationalRequest`.
    ///
    /// Dock icon will bounce once.
    Informational,
}

impl Default for RequestUserAttentionType {
    fn default() -> Self {
        RequestUserAttentionType::Critical
    }
}

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

    /// Request user attention, causing the application's dock icon to bounce.
    /// Note that this has no effect if the application is already focused.
    fn request_user_attention(&self, request_type: RequestUserAttentionType);

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
    fn request_user_attention(&self, request_type: RequestUserAttentionType) {
        self.window.request_user_attention(request_type)
    }

    #[inline]
    fn simple_fullscreen(&self) -> bool {
        self.window.simple_fullscreen()
    }

    #[inline]
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool {
        self.window.set_simple_fullscreen(fullscreen)
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
    /// Sets the activation policy for the window being built.
    fn with_activation_policy(self, activation_policy: ActivationPolicy) -> WindowBuilder;
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
    fn with_resize_increments(self, increments: LogicalSize) -> WindowBuilder;
    fn with_disallow_hidpi(self, disallow_hidpi: bool) -> WindowBuilder;
}

impl WindowBuilderExtMacOS for WindowBuilder {
    #[inline]
    fn with_activation_policy(mut self, activation_policy: ActivationPolicy) -> WindowBuilder {
        self.platform_specific.activation_policy = activation_policy;
        self
    }

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
    fn with_resize_increments(mut self, increments: LogicalSize) -> WindowBuilder {
        self.platform_specific.resize_increments = Some(increments.into());
        self
    }

    #[inline]
    fn with_disallow_hidpi(mut self, disallow_hidpi: bool) -> WindowBuilder {
        self.platform_specific.disallow_hidpi = disallow_hidpi;
        self
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
