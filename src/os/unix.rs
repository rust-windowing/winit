#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

use std::sync::Arc;
use std::ptr;
use libc;
use Window;
use platform::Window2 as LinuxWindow;
use platform::{UnixBackend, UNIX_BACKEND};
use WindowBuilder;
use platform::x11::XConnection;
use platform::x11::ffi::XVisualInfo;

pub use platform::x11;

// TODO: do not expose XConnection
pub fn get_x11_xconnection() -> Option<Arc<XConnection>> {
    match *UNIX_BACKEND {
        UnixBackend::X(ref connec) => Some(connec.clone()),
        _ => None,
    }
}

/// Additional methods on `Window` that are specific to Unix.
pub trait WindowExt {
    /// Returns a pointer to the `Window` object of xlib that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_xlib_window(&self) -> Option<*mut libc::c_void>;

    /// Returns a pointer to the `Display` object of xlib that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_xlib_display(&self) -> Option<*mut libc::c_void>;

    fn get_xlib_screen_id(&self) -> Option<*mut libc::c_void>;

    fn get_xlib_xconnection(&self) -> Option<Arc<XConnection>>;

    fn send_xim_spot(&self, x: i16, y: i16);
    
    /// This function returns the underlying `xcb_connection_t` of an xlib `Display`.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_xcb_connection(&self) -> Option<*mut libc::c_void>;

    /// Returns a pointer to the `wl_surface` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_wayland_surface(&self) -> Option<*mut libc::c_void>;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_wayland_display(&self) -> Option<*mut libc::c_void>;
}

impl WindowExt for Window {
    #[inline]
    fn get_xlib_window(&self) -> Option<*mut libc::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xlib_window()),
            _ => None
        }
    }

    #[inline]
    fn get_xlib_display(&self) -> Option<*mut libc::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xlib_display()),
            _ => None
        }
    }

    fn get_xlib_screen_id(&self) -> Option<*mut libc::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xlib_screen_id()),
            _ => None
        }
    }

    fn get_xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xlib_xconnection()),
            _ => None
        }
    }

    fn get_xcb_connection(&self) -> Option<*mut libc::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xcb_connection()),
            _ => None
        }
    }

    fn send_xim_spot(&self, x: i16, y: i16) {
        if let LinuxWindow::X(ref w) = self.window {
            w.send_xim_spot(x, y);
        }
    }

    #[inline]
    fn get_wayland_surface(&self) -> Option<*mut libc::c_void> {
        use wayland_client::Proxy;
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.get_surface().ptr() as *mut _),
            _ => None
        }
    }

    #[inline]
    fn get_wayland_display(&self) -> Option<*mut libc::c_void> {
        use wayland_client::Proxy;
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.get_display().ptr() as *mut _),
            _ => None
        }
    }
}

/// Additional methods on `WindowBuilder` that are specific to Unix.
pub trait WindowBuilderExt {
    fn with_x11_visual<T>(self, visual_infos: *const T) -> WindowBuilder;
    fn with_x11_screen(self, screen_id: i32) -> WindowBuilder;
}

impl WindowBuilderExt for WindowBuilder {
    #[inline]
    fn with_x11_visual<T>(mut self, visual_infos: *const T) -> WindowBuilder {
        self.platform_specific.visual_infos = Some(
            unsafe { ptr::read(visual_infos as *const XVisualInfo) }
        );
        self
    }

    #[inline]
    fn with_x11_screen(mut self, screen_id: i32) -> WindowBuilder {
        self.platform_specific.screen_id = Some(screen_id);
        self
    }
}
