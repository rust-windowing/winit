use std::os::raw;
use std::{ptr, sync::Arc};

use crate::{
    event_loop::{EventLoopBuilder, EventLoopWindowTarget},
    monitor::MonitorHandle,
    window::{Window, WindowBuilder},
};

use crate::dpi::Size;
use crate::platform_impl::{
    x11::ffi::XVisualInfo, x11::XConnection, ApplicationName, Backend,
    EventLoopWindowTarget as LinuxEventLoopWindowTarget, Window as LinuxWindow, XLIB_ERROR_HOOKS,
};

// TODO: stupid hack so that glutin can do its work
#[doc(hidden)]
pub use crate::platform_impl::x11;
pub use crate::platform_impl::{x11::util::WindowType as XWindowType, XNotSupported};

/// The first argument in the provided hook will be the pointer to `XDisplay`
/// and the second one the pointer to [`XErrorEvent`]. The returned `bool` is an
/// indicator whether the error was handled by the callback.
///
/// [`XErrorEvent`]: https://linux.die.net/man/3/xerrorevent
pub type XlibErrorHook =
    Box<dyn Fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> bool + Send + Sync>;

/// Hook to winit's xlib error handling callback.
///
/// This method is provided as a safe way to handle the errors comming from X11 when using xlib
/// in external crates, like glutin for GLX access. Trying to handle errors by speculating with
/// `XSetErrorHandler` is [`unsafe`].
///
/// [`unsafe`]: https://www.remlab.net/op/xlib.shtml
#[inline]
pub fn register_xlib_error_hook(hook: XlibErrorHook) {
    // Append new hook.
    unsafe {
        XLIB_ERROR_HOOKS.lock().unwrap().push(hook);
    }
}

/// Additional methods on [`EventLoopWindowTarget`] that are specific to X11.
pub trait EventLoopWindowTargetExtX11 {
    /// True if the [`EventLoopWindowTarget`] uses X11.
    fn is_x11(&self) -> bool;

    #[doc(hidden)]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>>;
}

impl<T> EventLoopWindowTargetExtX11 for EventLoopWindowTarget<T> {
    #[inline]
    fn is_x11(&self) -> bool {
        !self.p.is_wayland()
    }

    #[inline]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        match self.p {
            LinuxEventLoopWindowTarget::X(ref e) => Some(e.x_connection().clone()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }
}

/// Additional methods on [`EventLoopBuilder`] that are specific to X11.
pub trait EventLoopBuilderExtX11 {
    /// Force using X11.
    fn with_x11(&mut self) -> &mut Self;

    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;
}

impl<T> EventLoopBuilderExtX11 for EventLoopBuilder<T> {
    #[inline]
    fn with_x11(&mut self) -> &mut Self {
        self.platform_specific.forced_backend = Some(Backend::X);
        self
    }

    #[inline]
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.platform_specific.any_thread = any_thread;
        self
    }
}

/// Additional methods on [`Window`] that are specific to X11.
pub trait WindowExtX11 {
    /// Returns the ID of the [`Window`] xlib object that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    fn xlib_window(&self) -> Option<raw::c_ulong>;

    /// Returns a pointer to the `Display` object of xlib that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn xlib_display(&self) -> Option<*mut raw::c_void>;

    fn xlib_screen_id(&self) -> Option<raw::c_int>;

    #[doc(hidden)]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>>;

    /// This function returns the underlying `xcb_connection_t` of an xlib `Display`.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn xcb_connection(&self) -> Option<*mut raw::c_void>;
}

impl WindowExtX11 for Window {
    #[inline]
    fn xlib_window(&self) -> Option<raw::c_ulong> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_window()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }

    #[inline]
    fn xlib_display(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_display()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }

    #[inline]
    fn xlib_screen_id(&self) -> Option<raw::c_int> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_screen_id()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }

    #[inline]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_xconnection()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }

    #[inline]
    fn xcb_connection(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xcb_connection()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }
}

/// Additional methods on [`WindowBuilder`] that are specific to X11.
pub trait WindowBuilderExtX11 {
    fn with_x11_visual<T>(self, visual_infos: *const T) -> Self;

    fn with_x11_screen(self, screen_id: i32) -> Self;

    /// Build window with the given `general` and `instance` names.
    ///
    /// The `general` sets general class of `WM_CLASS(STRING)`, while `instance` set the
    /// instance part of it. The resulted property looks like `WM_CLASS(STRING) = "general", "instance"`.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    fn with_name(self, general: impl Into<String>, instance: impl Into<String>) -> Self;

    /// Build window with override-redirect flag; defaults to false. Only relevant on X11.
    fn with_override_redirect(self, override_redirect: bool) -> Self;

    /// Build window with `_NET_WM_WINDOW_TYPE` hints; defaults to `Normal`. Only relevant on X11.
    fn with_x11_window_type(self, x11_window_type: Vec<XWindowType>) -> Self;

    /// Build window with `_GTK_THEME_VARIANT` hint set to the specified value. Currently only relevant on X11.
    fn with_gtk_theme_variant(self, variant: String) -> Self;

    /// Build window with resize increment hint. Only implemented on X11.
    ///
    /// ```
    /// # use winit::dpi::{LogicalSize, PhysicalSize};
    /// # use winit::window::WindowBuilder;
    /// # use winit::platform::x11::WindowBuilderExtX11;
    /// // Specify the size in logical dimensions like this:
    /// WindowBuilder::new().with_resize_increments(LogicalSize::new(400.0, 200.0));
    ///
    /// // Or specify the size in physical dimensions like this:
    /// WindowBuilder::new().with_resize_increments(PhysicalSize::new(400, 200));
    /// ```
    fn with_resize_increments<S: Into<Size>>(self, increments: S) -> Self;

    /// Build window with base size hint. Only implemented on X11.
    ///
    /// ```
    /// # use winit::dpi::{LogicalSize, PhysicalSize};
    /// # use winit::window::WindowBuilder;
    /// # use winit::platform::x11::WindowBuilderExtX11;
    /// // Specify the size in logical dimensions like this:
    /// WindowBuilder::new().with_base_size(LogicalSize::new(400.0, 200.0));
    ///
    /// // Or specify the size in physical dimensions like this:
    /// WindowBuilder::new().with_base_size(PhysicalSize::new(400, 200));
    /// ```
    fn with_base_size<S: Into<Size>>(self, base_size: S) -> Self;
}

impl WindowBuilderExtX11 for WindowBuilder {
    #[inline]
    fn with_x11_visual<T>(mut self, visual_infos: *const T) -> Self {
        {
            self.platform_specific.visual_infos =
                Some(unsafe { ptr::read(visual_infos as *const XVisualInfo) });
        }
        self
    }

    #[inline]
    fn with_x11_screen(mut self, screen_id: i32) -> Self {
        self.platform_specific.screen_id = Some(screen_id);
        self
    }

    #[inline]
    fn with_name(mut self, general: impl Into<String>, instance: impl Into<String>) -> Self {
        self.platform_specific.name = Some(ApplicationName::new(general.into(), instance.into()));
        self
    }

    #[inline]
    fn with_override_redirect(mut self, override_redirect: bool) -> Self {
        self.platform_specific.override_redirect = override_redirect;
        self
    }

    #[inline]
    fn with_x11_window_type(mut self, x11_window_types: Vec<XWindowType>) -> Self {
        self.platform_specific.x11_window_types = x11_window_types;
        self
    }

    #[inline]
    fn with_gtk_theme_variant(mut self, variant: String) -> Self {
        self.platform_specific.gtk_theme_variant = Some(variant);
        self
    }

    #[inline]
    fn with_resize_increments<S: Into<Size>>(mut self, increments: S) -> Self {
        self.platform_specific.resize_increments = Some(increments.into());
        self
    }

    #[inline]
    fn with_base_size<S: Into<Size>>(mut self, base_size: S) -> Self {
        self.platform_specific.base_size = Some(base_size.into());
        self
    }
}

/// Additional methods on `MonitorHandle` that are specific to X11.
pub trait MonitorHandleExtX11 {
    /// Returns the inner identifier of the monitor.
    fn native_id(&self) -> u32;
}

impl MonitorHandleExtX11 for MonitorHandle {
    #[inline]
    fn native_id(&self) -> u32 {
        self.inner.native_identifier()
    }
}
