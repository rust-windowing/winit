#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]

use std::{os::raw, ptr, sync::Arc};

use smithay_client_toolkit::window::{ButtonState, Theme};

use crate::{
    dpi::LogicalSize,
    event_loop::{EventLoop, EventLoopWindowTarget},
    monitor::MonitorHandle,
    window::{Window, WindowBuilder},
};

use crate::platform_impl::{
    x11::{ffi::XVisualInfo, XConnection},
    EventLoop as LinuxEventLoop, EventLoopWindowTarget as LinuxEventLoopWindowTarget,
    Window as LinuxWindow,
};

// TODO: stupid hack so that glutin can do its work
#[doc(hidden)]
pub use crate::platform_impl::x11;

pub use crate::platform_impl::{x11::util::WindowType as XWindowType, XNotSupported};

/// Theme for wayland client side decorations
///
/// Colors must be in ARGB8888 format
pub struct WaylandTheme {
    /// Primary color when the window is focused
    pub primary_active: [u8; 4],
    /// Primary color when the window is unfocused
    pub primary_inactive: [u8; 4],
    /// Secondary color when the window is focused
    pub secondary_active: [u8; 4],
    /// Secondary color when the window is unfocused
    pub secondary_inactive: [u8; 4],
    /// Close button color when hovered over
    pub close_button_hovered: [u8; 4],
    /// Close button color
    pub close_button: [u8; 4],
    /// Close button color when hovered over
    pub maximize_button_hovered: [u8; 4],
    /// Maximize button color
    pub maximize_button: [u8; 4],
    /// Minimize button color when hovered over
    pub minimize_button_hovered: [u8; 4],
    /// Minimize button color
    pub minimize_button: [u8; 4],
}

struct WaylandThemeObject(WaylandTheme);

impl Theme for WaylandThemeObject {
    fn get_primary_color(&self, active: bool) -> [u8; 4] {
        if active {
            self.0.primary_active
        } else {
            self.0.primary_inactive
        }
    }

    // Used for division line
    fn get_secondary_color(&self, active: bool) -> [u8; 4] {
        if active {
            self.0.secondary_active
        } else {
            self.0.secondary_inactive
        }
    }

    fn get_close_button_color(&self, state: ButtonState) -> [u8; 4] {
        match state {
            ButtonState::Hovered => self.0.close_button_hovered,
            _ => self.0.close_button,
        }
    }

    fn get_maximize_button_color(&self, state: ButtonState) -> [u8; 4] {
        match state {
            ButtonState::Hovered => self.0.maximize_button_hovered,
            _ => self.0.maximize_button,
        }
    }

    fn get_minimize_button_color(&self, state: ButtonState) -> [u8; 4] {
        match state {
            ButtonState::Hovered => self.0.minimize_button_hovered,
            _ => self.0.minimize_button,
        }
    }
}

/// Additional methods on `EventLoopWindowTarget` that are specific to Unix.
pub trait EventLoopWindowTargetExtUnix {
    /// True if the `EventLoopWindowTarget` uses Wayland.
    fn is_wayland(&self) -> bool;
    ///
    /// True if the `EventLoopWindowTarget` uses X11.
    fn is_x11(&self) -> bool;

    #[doc(hidden)]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>>;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this
    /// `EventLoopWindowTarget`.
    ///
    /// Returns `None` if the `EventLoop` doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the winit `EventLoop` is destroyed.
    fn wayland_display(&self) -> Option<*mut raw::c_void>;
}

impl<T> EventLoopWindowTargetExtUnix for EventLoopWindowTarget<T> {
    #[inline]
    fn is_wayland(&self) -> bool {
        self.p.is_wayland()
    }

    #[inline]
    fn is_x11(&self) -> bool {
        !self.p.is_wayland()
    }

    #[inline]
    #[doc(hidden)]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        match self.p {
            LinuxEventLoopWindowTarget::X(ref e) => Some(e.x_connection().clone()),
            _ => None,
        }
    }

    #[inline]
    fn wayland_display(&self) -> Option<*mut raw::c_void> {
        match self.p {
            LinuxEventLoopWindowTarget::Wayland(ref p) => {
                Some(p.display().get_display_ptr() as *mut _)
            }
            _ => None,
        }
    }
}

/// Additional methods on `EventLoop` that are specific to Unix.
pub trait EventLoopExtUnix {
    /// Builds a new `EventLoops` that is forced to use X11.
    fn new_x11() -> Result<Self, XNotSupported>
    where
        Self: Sized;

    /// Builds a new `EventLoop` that is forced to use Wayland.
    fn new_wayland() -> Self
    where
        Self: Sized;
}

impl<T> EventLoopExtUnix for EventLoop<T> {
    #[inline]
    fn new_x11() -> Result<Self, XNotSupported> {
        LinuxEventLoop::new_x11().map(|ev| EventLoop {
            event_loop: ev,
            _marker: ::std::marker::PhantomData,
        })
    }

    #[inline]
    fn new_wayland() -> Self {
        EventLoop {
            event_loop: match LinuxEventLoop::new_wayland() {
                Ok(e) => e,
                Err(_) => panic!(), // TODO: propagate
            },
            _marker: ::std::marker::PhantomData,
        }
    }
}

/// Additional methods on `Window` that are specific to Unix.
pub trait WindowExtUnix {
    /// Returns the ID of the `Window` xlib object that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    fn xlib_window(&self) -> Option<raw::c_ulong>;

    /// Returns a pointer to the `Display` object of xlib that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn xlib_display(&self) -> Option<*mut raw::c_void>;

    fn xlib_screen_id(&self) -> Option<raw::c_int>;

    #[doc(hidden)]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>>;

    /// Set window urgency hint (`XUrgencyHint`). Only relevant on X.
    fn set_urgent(&self, is_urgent: bool);

    /// This function returns the underlying `xcb_connection_t` of an xlib `Display`.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn xcb_connection(&self) -> Option<*mut raw::c_void>;

    /// Returns a pointer to the `wl_surface` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn wayland_surface(&self) -> Option<*mut raw::c_void>;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn wayland_display(&self) -> Option<*mut raw::c_void>;

    /// Sets the color theme of the client side window decorations on wayland
    fn set_wayland_theme(&self, theme: WaylandTheme);

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
    fn xlib_window(&self) -> Option<raw::c_ulong> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_window()),
            _ => None,
        }
    }

    #[inline]
    fn xlib_display(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_display()),
            _ => None,
        }
    }

    #[inline]
    fn xlib_screen_id(&self) -> Option<raw::c_int> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_screen_id()),
            _ => None,
        }
    }

    #[inline]
    #[doc(hidden)]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_xconnection()),
            _ => None,
        }
    }

    #[inline]
    fn xcb_connection(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xcb_connection()),
            _ => None,
        }
    }

    #[inline]
    fn set_urgent(&self, is_urgent: bool) {
        if let LinuxWindow::X(ref w) = self.window {
            w.set_urgent(is_urgent);
        }
    }

    #[inline]
    fn wayland_surface(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.surface().as_ref().c_ptr() as *mut _),
            _ => None,
        }
    }

    #[inline]
    fn wayland_display(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.display().as_ref().c_ptr() as *mut _),
            _ => None,
        }
    }

    #[inline]
    fn set_wayland_theme(&self, theme: WaylandTheme) {
        match self.window {
            LinuxWindow::Wayland(ref w) => w.set_theme(WaylandThemeObject(theme)),
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
    fn with_x11_visual<T>(self, visual_infos: *const T) -> WindowBuilder;
    fn with_x11_screen(self, screen_id: i32) -> WindowBuilder;

    /// Build window with `WM_CLASS` hint; defaults to the name of the binary. Only relevant on X11.
    fn with_class(self, class: String, instance: String) -> WindowBuilder;
    /// Build window with override-redirect flag; defaults to false. Only relevant on X11.
    fn with_override_redirect(self, override_redirect: bool) -> WindowBuilder;
    /// Build window with `_NET_WM_WINDOW_TYPE` hint; defaults to `Normal`. Only relevant on X11.
    fn with_x11_window_type(self, x11_window_type: XWindowType) -> WindowBuilder;
    /// Build window with `_GTK_THEME_VARIANT` hint set to the specified value. Currently only relevant on X11.
    fn with_gtk_theme_variant(self, variant: String) -> WindowBuilder;
    /// Build window with resize increment hint. Only implemented on X11.
    fn with_resize_increments(self, increments: LogicalSize) -> WindowBuilder;
    /// Build window with base size hint. Only implemented on X11.
    fn with_base_size(self, base_size: LogicalSize) -> WindowBuilder;

    /// Build window with a given application ID. It should match the `.desktop` file distributed with
    /// your program. Only relevant on Wayland.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    fn with_app_id(self, app_id: String) -> WindowBuilder;
}

impl WindowBuilderExtUnix for WindowBuilder {
    #[inline]
    fn with_x11_visual<T>(mut self, visual_infos: *const T) -> WindowBuilder {
        self.platform_specific.visual_infos =
            Some(unsafe { ptr::read(visual_infos as *const XVisualInfo) });
        self
    }

    #[inline]
    fn with_x11_screen(mut self, screen_id: i32) -> WindowBuilder {
        self.platform_specific.screen_id = Some(screen_id);
        self
    }

    #[inline]
    fn with_class(mut self, instance: String, class: String) -> WindowBuilder {
        self.platform_specific.class = Some((instance, class));
        self
    }

    #[inline]
    fn with_override_redirect(mut self, override_redirect: bool) -> WindowBuilder {
        self.platform_specific.override_redirect = override_redirect;
        self
    }

    #[inline]
    fn with_x11_window_type(mut self, x11_window_type: XWindowType) -> WindowBuilder {
        self.platform_specific.x11_window_type = x11_window_type;
        self
    }

    #[inline]
    fn with_resize_increments(mut self, increments: LogicalSize) -> WindowBuilder {
        self.platform_specific.resize_increments = Some(increments.into());
        self
    }

    #[inline]
    fn with_base_size(mut self, base_size: LogicalSize) -> WindowBuilder {
        self.platform_specific.base_size = Some(base_size.into());
        self
    }

    #[inline]
    fn with_gtk_theme_variant(mut self, variant: String) -> WindowBuilder {
        self.platform_specific.gtk_theme_variant = Some(variant);
        self
    }

    #[inline]
    fn with_app_id(mut self, app_id: String) -> WindowBuilder {
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
