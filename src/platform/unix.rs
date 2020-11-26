#![cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

use std::os::raw;
#[cfg(feature = "x11")]
use std::{ptr, sync::Arc};

use crate::{
    event_loop::{EventLoop, EventLoopWindowTarget},
    monitor::MonitorHandle,
    window::{Window, WindowBuilder},
};

#[cfg(feature = "x11")]
use crate::dpi::Size;
#[cfg(feature = "x11")]
use crate::platform_impl::x11::{ffi::XVisualInfo, XConnection};
use crate::platform_impl::{
    EventLoop as LinuxEventLoop, EventLoopWindowTarget as LinuxEventLoopWindowTarget,
    Window as LinuxWindow,
};

// TODO: stupid hack so that glutin can do its work
#[doc(hidden)]
#[cfg(feature = "x11")]
pub use crate::platform_impl::x11;
#[cfg(feature = "x11")]
pub use crate::platform_impl::{x11::util::WindowType as XWindowType, XNotSupported};

/// Additional methods on `EventLoopWindowTarget` that are specific to Unix.
pub trait EventLoopWindowTargetExtUnix {
    /// True if the `EventLoopWindowTarget` uses Wayland.
    #[cfg(feature = "wayland")]
    fn is_wayland(&self) -> bool;

    /// True if the `EventLoopWindowTarget` uses X11.
    #[cfg(feature = "x11")]
    fn is_x11(&self) -> bool;

    #[doc(hidden)]
    #[cfg(feature = "x11")]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>>;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this
    /// `EventLoopWindowTarget`.
    ///
    /// Returns `None` if the `EventLoop` doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the winit `EventLoop` is destroyed.
    #[cfg(feature = "wayland")]
    fn wayland_display(&self) -> Option<*mut raw::c_void>;
}

impl<T> EventLoopWindowTargetExtUnix for EventLoopWindowTarget<T> {
    #[inline]
    #[cfg(feature = "wayland")]
    fn is_wayland(&self) -> bool {
        self.p.is_wayland()
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn is_x11(&self) -> bool {
        !self.p.is_wayland()
    }

    #[inline]
    #[doc(hidden)]
    #[cfg(feature = "x11")]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        match self.p {
            LinuxEventLoopWindowTarget::X(ref e) => Some(e.x_connection().clone()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "wayland")]
    fn wayland_display(&self) -> Option<*mut raw::c_void> {
        match self.p {
            LinuxEventLoopWindowTarget::Wayland(ref p) => {
                Some(p.display().get_display_ptr() as *mut _)
            }
            #[cfg(feature = "x11")]
            _ => None,
        }
    }
}

/// Additional methods on `EventLoop` that are specific to Unix.
pub trait EventLoopExtUnix {
    /// Builds a new `EventLoop` that is forced to use X11.
    ///
    /// # Panics
    ///
    /// If called outside the main thread. To initialize an X11 event loop outside
    /// the main thread, use [`new_x11_any_thread`](#tymethod.new_x11_any_thread).
    #[cfg(feature = "x11")]
    fn new_x11() -> Result<Self, XNotSupported>
    where
        Self: Sized;

    /// Builds a new `EventLoop` that is forced to use Wayland.
    ///
    /// # Panics
    ///
    /// If called outside the main thread. To initialize a Wayland event loop outside
    /// the main thread, use [`new_wayland_any_thread`](#tymethod.new_wayland_any_thread).
    #[cfg(feature = "wayland")]
    fn new_wayland() -> Self
    where
        Self: Sized;

    /// Builds a new `EventLoop` on any thread.
    ///
    /// This method bypasses the cross-platform compatibility requirement
    /// that `EventLoop` be created on the main thread.
    fn new_any_thread() -> Self
    where
        Self: Sized;

    /// Builds a new X11 `EventLoop` on any thread.
    ///
    /// This method bypasses the cross-platform compatibility requirement
    /// that `EventLoop` be created on the main thread.
    #[cfg(feature = "x11")]
    fn new_x11_any_thread() -> Result<Self, XNotSupported>
    where
        Self: Sized;

    /// Builds a new Wayland `EventLoop` on any thread.
    ///
    /// This method bypasses the cross-platform compatibility requirement
    /// that `EventLoop` be created on the main thread.
    #[cfg(feature = "wayland")]
    fn new_wayland_any_thread() -> Self
    where
        Self: Sized;
}

fn wrap_ev<T>(event_loop: LinuxEventLoop<T>) -> EventLoop<T> {
    EventLoop {
        event_loop,
        _marker: std::marker::PhantomData,
    }
}

impl<T> EventLoopExtUnix for EventLoop<T> {
    #[inline]
    fn new_any_thread() -> Self {
        wrap_ev(LinuxEventLoop::new_any_thread())
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn new_x11_any_thread() -> Result<Self, XNotSupported> {
        LinuxEventLoop::new_x11_any_thread().map(wrap_ev)
    }

    #[inline]
    #[cfg(feature = "wayland")]
    fn new_wayland_any_thread() -> Self {
        wrap_ev(
            LinuxEventLoop::new_wayland_any_thread()
                // TODO: propagate
                .expect("failed to open Wayland connection"),
        )
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn new_x11() -> Result<Self, XNotSupported> {
        LinuxEventLoop::new_x11().map(wrap_ev)
    }

    #[inline]
    #[cfg(feature = "wayland")]
    fn new_wayland() -> Self {
        wrap_ev(
            LinuxEventLoop::new_wayland()
                // TODO: propagate
                .expect("failed to open Wayland connection"),
        )
    }
}

/// Additional methods on `Window` that are specific to Unix.
pub trait WindowExtUnix {
    /// Returns the ID of the `Window` xlib object that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    #[cfg(feature = "x11")]
    fn xlib_window(&self) -> Option<raw::c_ulong>;

    /// Returns a pointer to the `Display` object of xlib that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    #[cfg(feature = "x11")]
    fn xlib_display(&self) -> Option<*mut raw::c_void>;

    #[cfg(feature = "x11")]
    fn xlib_screen_id(&self) -> Option<raw::c_int>;

    #[doc(hidden)]
    #[cfg(feature = "x11")]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>>;

    /// This function returns the underlying `xcb_connection_t` of an xlib `Display`.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    #[cfg(feature = "x11")]
    fn xcb_connection(&self) -> Option<*mut raw::c_void>;

    /// Returns a pointer to the `wl_surface` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    #[cfg(feature = "wayland")]
    fn wayland_surface(&self) -> Option<*mut raw::c_void>;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    #[cfg(feature = "wayland")]
    fn wayland_display(&self) -> Option<*mut raw::c_void>;

    /// Sets the color theme of the client side window decorations on wayland
    #[cfg(feature = "wayland")]
    fn set_wayland_theme<T: Theme>(&self, theme: T);

    /// Check if the window is ready for drawing
    ///
    /// It is a remnant of a previous implementation detail for the
    /// wayland backend, and is no longer relevant.
    ///
    /// Always return true.
    #[deprecated]
    fn is_ready(&self) -> bool;
}

impl WindowExtUnix for Window {
    #[inline]
    #[cfg(feature = "x11")]
    fn xlib_window(&self) -> Option<raw::c_ulong> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_window()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn xlib_display(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_display()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn xlib_screen_id(&self) -> Option<raw::c_int> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_screen_id()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }

    #[inline]
    #[doc(hidden)]
    #[cfg(feature = "x11")]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_xconnection()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn xcb_connection(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xcb_connection()),
            #[cfg(feature = "wayland")]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "wayland")]
    fn wayland_surface(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.surface().as_ref().c_ptr() as *mut _),
            #[cfg(feature = "x11")]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "wayland")]
    fn wayland_display(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.display().get_display_ptr() as *mut _),
            #[cfg(feature = "x11")]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "wayland")]
    fn set_wayland_theme<T: Theme>(&self, theme: T) {
        match self.window {
            LinuxWindow::Wayland(ref w) => w.set_theme(theme),
            #[cfg(feature = "x11")]
            _ => {}
        }
    }

    #[inline]
    fn is_ready(&self) -> bool {
        true
    }
}

/// Additional methods on `WindowBuilder` that are specific to Unix.
pub trait WindowBuilderExtUnix {
    #[cfg(feature = "x11")]
    fn with_x11_visual<T>(self, visual_infos: *const T) -> Self;
    #[cfg(feature = "x11")]
    fn with_x11_screen(self, screen_id: i32) -> Self;

    /// Build window with `WM_CLASS` hint; defaults to the name of the binary. Only relevant on X11.
    #[cfg(feature = "x11")]
    fn with_class(self, class: String, instance: String) -> Self;
    /// Build window with override-redirect flag; defaults to false. Only relevant on X11.
    #[cfg(feature = "x11")]
    fn with_override_redirect(self, override_redirect: bool) -> Self;
    /// Build window with `_NET_WM_WINDOW_TYPE` hints; defaults to `Normal`. Only relevant on X11.
    #[cfg(feature = "x11")]
    fn with_x11_window_type(self, x11_window_type: Vec<XWindowType>) -> Self;
    /// Build window with `_GTK_THEME_VARIANT` hint set to the specified value. Currently only relevant on X11.
    #[cfg(feature = "x11")]
    fn with_gtk_theme_variant(self, variant: String) -> Self;
    /// Build window with resize increment hint. Only implemented on X11.
    #[cfg(feature = "x11")]
    fn with_resize_increments<S: Into<Size>>(self, increments: S) -> Self;
    /// Build window with base size hint. Only implemented on X11.
    #[cfg(feature = "x11")]
    fn with_base_size<S: Into<Size>>(self, base_size: S) -> Self;

    /// Build window with a given application ID. It should match the `.desktop` file distributed with
    /// your program. Only relevant on Wayland.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    #[cfg(feature = "wayland")]
    fn with_app_id(self, app_id: String) -> Self;
}

impl WindowBuilderExtUnix for WindowBuilder {
    #[inline]
    #[cfg(feature = "x11")]
    fn with_x11_visual<T>(mut self, visual_infos: *const T) -> Self {
        {
            self.platform_specific.visual_infos =
                Some(unsafe { ptr::read(visual_infos as *const XVisualInfo) });
        }
        self
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn with_x11_screen(mut self, screen_id: i32) -> Self {
        self.platform_specific.screen_id = Some(screen_id);
        self
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn with_class(mut self, instance: String, class: String) -> Self {
        self.platform_specific.class = Some((instance, class));
        self
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn with_override_redirect(mut self, override_redirect: bool) -> Self {
        self.platform_specific.override_redirect = override_redirect;
        self
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn with_x11_window_type(mut self, x11_window_types: Vec<XWindowType>) -> Self {
        self.platform_specific.x11_window_types = x11_window_types;
        self
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn with_gtk_theme_variant(mut self, variant: String) -> Self {
        self.platform_specific.gtk_theme_variant = Some(variant);
        self
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn with_resize_increments<S: Into<Size>>(mut self, increments: S) -> Self {
        self.platform_specific.resize_increments = Some(increments.into());
        self
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn with_base_size<S: Into<Size>>(mut self, base_size: S) -> Self {
        self.platform_specific.base_size = Some(base_size.into());
        self
    }

    #[inline]
    #[cfg(feature = "wayland")]
    fn with_app_id(mut self, app_id: String) -> Self {
        self.platform_specific.app_id = Some(app_id);
        self
    }
}

/// Additional methods on `MonitorHandle` that are specific to Linux.
pub trait MonitorHandleExtUnix {
    /// Returns the inner identifier of the monitor.
    fn native_id(&self) -> u32;
}

impl MonitorHandleExtUnix for MonitorHandle {
    #[inline]
    fn native_id(&self) -> u32 {
        self.inner.native_identifier()
    }
}

/// A theme for a Wayland's client side decorations.
#[cfg(feature = "wayland")]
pub trait Theme: Send + 'static {
    /// Title bar color.
    fn element_color(&self, element: Element, window_active: bool) -> ARGBColor;

    /// Color for a given button part.
    fn button_color(
        &self,
        button: Button,
        state: ButtonState,
        foreground: bool,
        window_active: bool,
    ) -> ARGBColor;

    /// Font name and the size for the title bar.
    ///
    /// By default the font is `sans-serif` at the size of 17.
    ///
    /// Returning `None` means that title won't be drawn.
    fn font(&self) -> Option<(String, f32)> {
        // Not having any title isn't something desirable for the users, so setting it to
        // something generic.
        Some((String::from("sans-serif"), 17.))
    }
}

/// A button on Wayland's client side decorations.
#[cfg(feature = "wayland")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// Button that maximizes the window.
    Maximize,

    /// Button that minimizes the window.
    Minimize,

    /// Button that closes the window.
    Close,
}

/// A button state of the button on Wayland's client side decorations.
#[cfg(feature = "wayland")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    /// Button is being hovered over by pointer.
    Hovered,
    /// Button is not being hovered over by pointer.
    Idle,
    /// Button is disabled.
    Disabled,
}

#[cfg(feature = "wayland")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    /// Bar itself.
    Bar,

    /// Separator between window and title bar.
    Separator,

    /// Title bar text.
    Text,
}

#[cfg(feature = "wayland")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ARGBColor {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
