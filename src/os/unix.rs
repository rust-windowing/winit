#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

use std::sync::Arc;
use libc;
use Window;
use platform::Window as LinuxWindow;
use WindowBuilder;
use api::x11::XConnection;

use wayland_client::protocol::wl_display::WlDisplay;
use wayland_client::protocol::wl_surface::WlSurface;

pub use api::x11;

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

    /// Returns a reference to the `WlSurface` object of wayland that is used by this window.
    ///
    /// For use with the `wayland-client` crate.
    ///
    /// **This function is not part of winit's public API.**
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    fn get_wayland_client_surface(&self) -> Option<&WlSurface>;

    /// Returns a pointer to the `WlDisplay` object of wayland that is used by this window.
    ///
    /// For use with the `wayland-client` crate.
    ///
    /// **This function is not part of winit's public API.**
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    fn get_wayland_client_display(&self) -> Option<&WlDisplay>;
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

    #[inline]
    fn get_wayland_surface(&self) -> Option<*mut libc::c_void> {
        use wayland_client::Proxy;
        self.get_wayland_client_surface().map(|p| p.ptr() as *mut _)
    }


    #[inline]
    fn get_wayland_display(&self) -> Option<*mut libc::c_void> {
        use wayland_client::Proxy;
        self.get_wayland_client_display().map(|p| p.ptr() as *mut _)
    }

    #[inline]
    fn get_wayland_client_surface(&self) -> Option<&WlSurface> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.get_surface()),
            _ => None
        }
    }

    #[inline]
    fn get_wayland_client_display(&self) -> Option<&WlDisplay> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.get_display()),
            _ => None
        }
    }
}

/// Additional methods on `WindowBuilder` that are specific to Unix.
pub trait WindowBuilderExt {

}

impl WindowBuilderExt for WindowBuilder {
}
