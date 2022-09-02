use std::os::raw::c_void;

use crate::{
    dpi::LogicalSize,
    event_loop::{EventLoopBuilder, EventLoopWindowTarget},
    monitor::MonitorHandle,
    window::{Window, WindowBuilder},
};

/// Additional methods on [`Window`] that are specific to MacOS.
pub trait WindowExtMacOS {
    /// Returns a pointer to the cocoa `NSWindow` that is used by this window.
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn ns_window(&self) -> *mut c_void;

    /// Returns a pointer to the cocoa `NSView` that is used by this window.
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Additional methods on [`WindowBuilder`] that are specific to MacOS.
///
/// **Note:** Properties dealing with the titlebar will be overwritten by the [`WindowBuilder::with_decorations`] method:
/// - `with_titlebar_transparent`
/// - `with_title_hidden`
/// - `with_titlebar_hidden`
/// - `with_titlebar_buttons_hidden`
/// - `with_fullsize_content_view`
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
        self.platform_specific.resize_increments = Some(increments);
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

pub trait EventLoopBuilderExtMacOS {
    /// Sets the activation policy for the application.
    ///
    /// It is set to [`ActivationPolicy::Regular`] by default.
    ///
    /// # Example
    ///
    /// Set the activation policy to "accessory".
    ///
    /// ```
    /// use winit::event_loop::EventLoopBuilder;
    /// #[cfg(target_os = "macos")]
    /// use winit::platform::macos::{EventLoopBuilderExtMacOS, ActivationPolicy};
    ///
    /// let mut builder = EventLoopBuilder::new();
    /// #[cfg(target_os = "macos")]
    /// builder.with_activation_policy(ActivationPolicy::Accessory);
    /// # if false { // We can't test this part
    /// let event_loop = builder.build();
    /// # }
    /// ```
    fn with_activation_policy(&mut self, activation_policy: ActivationPolicy) -> &mut Self;

    /// Used to control whether a default menubar menu is created.
    ///
    /// Menu creation is enabled by default.
    ///
    /// # Example
    ///
    /// Disable creating a default menubar.
    ///
    /// ```
    /// use winit::event_loop::EventLoopBuilder;
    /// #[cfg(target_os = "macos")]
    /// use winit::platform::macos::EventLoopBuilderExtMacOS;
    ///
    /// let mut builder = EventLoopBuilder::new();
    /// #[cfg(target_os = "macos")]
    /// builder.with_default_menu(false);
    /// # if false { // We can't test this part
    /// let event_loop = builder.build();
    /// # }
    /// ```
    fn with_default_menu(&mut self, enable: bool) -> &mut Self;
}

impl<T> EventLoopBuilderExtMacOS for EventLoopBuilder<T> {
    #[inline]
    fn with_activation_policy(&mut self, activation_policy: ActivationPolicy) -> &mut Self {
        self.platform_specific.activation_policy = activation_policy;
        self
    }

    #[inline]
    fn with_default_menu(&mut self, enable: bool) -> &mut Self {
        self.platform_specific.default_menu = enable;
        self
    }
}

/// Additional methods on [`MonitorHandle`] that are specific to MacOS.
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

/// Additional methods on [`EventLoopWindowTarget`] that are specific to macOS.
pub trait EventLoopWindowTargetExtMacOS {
    /// Hide the entire application. In most applications this is typically triggered with Command-H.
    fn hide_application(&self);
    /// Hide the other applications. In most applications this is typically triggered with Command+Option-H.
    fn hide_other_applications(&self);
}

impl<T> EventLoopWindowTargetExtMacOS for EventLoopWindowTarget<T> {
    fn hide_application(&self) {
        self.p.hide_application()
    }

    fn hide_other_applications(&self) {
        self.p.hide_other_applications()
    }
}
