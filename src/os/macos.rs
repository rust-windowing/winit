#![cfg(target_os = "macos")]

use std::convert::From;
use std::os::raw::c_void;
use cocoa::appkit::NSApplicationActivationPolicy;
use {LogicalSize, MonitorId, Window, WindowBuilder};

/// Additional methods on `Window` that are specific to MacOS.
pub trait WindowExt {
    /// Returns a pointer to the cocoa `NSWindow` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn get_nswindow(&self) -> *mut c_void;

    /// Returns a pointer to the cocoa `NSView` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn get_nsview(&self) -> *mut c_void;

    /// For windows created with the [blurred](WindowBuilder::with_blur) option,
    /// this controls the appearance of the blur effect.
    /// 
    /// Marked as unsafe because depending on the version of macOS and the `BlurMaterial` variant passed,
    /// this might cause a crash.
    unsafe fn set_blur_material(&self, material: BlurMaterial);
}

impl WindowExt for Window {
    #[inline]
    fn get_nswindow(&self) -> *mut c_void {
        self.window.get_nswindow()
    }

    #[inline]
    fn get_nsview(&self) -> *mut c_void {
        self.window.get_nsview()
    }

    #[inline]
    unsafe fn set_blur_material(&self, material: BlurMaterial) {
        self.window.set_blur_material(material);
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

impl From<ActivationPolicy> for NSApplicationActivationPolicy {
    fn from(activation_policy: ActivationPolicy) -> Self {
        match activation_policy {
            ActivationPolicy::Regular =>
                NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
            ActivationPolicy::Accessory =>
                NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory,
            ActivationPolicy::Prohibited =>
                NSApplicationActivationPolicy::NSApplicationActivationPolicyProhibited,
        }
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
pub trait WindowBuilderExt {
    /// Sets the activation policy for the window being built.
    fn with_activation_policy(self, activation_policy: ActivationPolicy) -> WindowBuilder;
    /// Enables click-and-drag behavior for the entire window, not just the titlebar.
    fn with_movable_by_window_background(self, movable_by_window_background: bool) -> WindowBuilder;
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
}

impl WindowBuilderExt for WindowBuilder {
    #[inline]
    fn with_activation_policy(mut self, activation_policy: ActivationPolicy) -> WindowBuilder {
        self.platform_specific.activation_policy = activation_policy;
        self
    }

    #[inline]
    fn with_movable_by_window_background(mut self, movable_by_window_background: bool) -> WindowBuilder {
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
}

/// Additional methods on `MonitorId` that are specific to MacOS.
pub trait MonitorIdExt {
    /// Returns the identifier of the monitor for Cocoa.
    fn native_id(&self) -> u32;
    /// Returns a pointer to the NSScreen representing this monitor.
    fn get_nsscreen(&self) -> Option<*mut c_void>;
}

impl MonitorIdExt for MonitorId {
    #[inline]
    fn native_id(&self) -> u32 {
        self.inner.get_native_identifier()
    }

    fn get_nsscreen(&self) -> Option<*mut c_void> {
        self.inner.get_nsscreen().map(|s| s as *mut c_void)
    }
}

/// Enumeration of all possible blur materials for macOS. Applies to macOS SDK 10.10+.
/// 
/// Not all versions of macOS support all the variants listed here.
/// Check [Apple's documentation](https://developer.apple.com/documentation/appkit/nsvisualeffectview/material)
/// to find out what your target version supports.
/// The behaviour for using a material which is not supported depends how it is implemented in cocoa,
/// but will most likely cause a crash.
#[repr(i64)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BlurMaterial {
    /// A default material for the view’s effective appearance.
    AppearanceBased = 0,
    /// A material with a light effect.
    Light = 1,
    /// A material with a dark effect.
    Dark = 2,
    /// The material for a window’s titlebar.
    Titlebar = 3,
    /// The material used to indicate a selection.
    Selection = 4,
    /// The material for menus.
    Menu = 5,
    /// The material for the background of popover windows.
    Popover = 6,
    /// The material for the background of window sidebars.
    Sidebar = 7,
    /// A material with a medium-light effect.
    MediumLight = 8,
    /// A material with an ultra-dark effect.
    UltraDark = 9,
    /// The material for in-line header or footer views.
    HeaderView = 10,
    /// The material for the background of sheet windows.
    Sheet = 11,
    /// The material for the background of opaque windows.
    WindowBackground = 12,
    /// The material for the background of heads-up display (HUD) windows.
    HudWindow = 13,
    /// The material for the background of a full-screen modal interface.
    FullScreenUi = 15,
    /// The material for the background of a tool tip.
    ToolTip = 17,
    /// The material for the background of opaque content.
    ContentBackground = 18,
    /// The material for under a window's background.
    UnderWindowBackground = 21,
    /// The material for the area behind the pages of a document.
    UnderPageBackground = 22,
}