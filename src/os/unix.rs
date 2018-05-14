#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

use std::os::raw;
use std::sync::Arc;
use std::ptr;
use EventsLoop;
use MonitorId;
use Window;
use platform::EventsLoop as LinuxEventsLoop;
use platform::Window as LinuxWindow;
use WindowBuilder;
use platform::x11::XConnection;
use platform::x11::ffi::XVisualInfo;

// TODO: stupid hack so that glutin can do its work
#[doc(hidden)]
pub use platform::x11;

pub use platform::XNotSupported;

/// Additional methods on `EventsLoop` that are specific to Linux.
pub trait EventsLoopExt {
    /// Builds a new `EventsLoop` that is forced to use X11.
    fn new_x11() -> Result<Self, XNotSupported>
        where Self: Sized;

    /// Builds a new `EventsLoop` that is forced to use Wayland.
    fn new_wayland() -> Self
        where Self: Sized;

    /// True if the `EventsLoop` uses Wayland.
    fn is_wayland(&self) -> bool;

    /// True if the `EventsLoop` uses X11.
    fn is_x11(&self) -> bool;

    #[doc(hidden)]
    fn get_xlib_xconnection(&self) -> Option<Arc<XConnection>>;
}

impl EventsLoopExt for EventsLoop {
    #[inline]
    fn new_x11() -> Result<Self, XNotSupported> {
        LinuxEventsLoop::new_x11().map(|ev|
            EventsLoop {
                events_loop: ev,
                _marker: ::std::marker::PhantomData,
            }
        )
    }

    #[inline]
    fn new_wayland() -> Self {
        EventsLoop {
            events_loop: match LinuxEventsLoop::new_wayland() {
                Ok(e) => e,
                Err(_) => panic!()      // TODO: propagate
            },
            _marker: ::std::marker::PhantomData,
        }
    }

    #[inline]
    fn is_wayland(&self) -> bool {
        self.events_loop.is_wayland()
    }

    #[inline]
    fn is_x11(&self) -> bool {
        !self.events_loop.is_wayland()
    }

    #[inline]
    fn get_xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        self.events_loop.x_connection().cloned()
    }
}

/// Additional methods on `Window` that are specific to Unix.
pub trait WindowExt {
    /// Returns the ID of the `Window` xlib object that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    fn get_xlib_window(&self) -> Option<raw::c_ulong>;

    /// Returns a pointer to the `Display` object of xlib that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_xlib_display(&self) -> Option<*mut raw::c_void>;

    fn get_xlib_screen_id(&self) -> Option<raw::c_int>;

    fn get_xlib_xconnection(&self) -> Option<Arc<XConnection>>;

    /// This function returns the underlying `xcb_connection_t` of an xlib `Display`.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_xcb_connection(&self) -> Option<*mut raw::c_void>;

    /// Returns a pointer to the `wl_surface` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_wayland_surface(&self) -> Option<*mut raw::c_void>;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    fn get_wayland_display(&self) -> Option<*mut raw::c_void>;

    /// Check if the window is ready for drawing
    ///
    /// It is a remnant of a previous implementation detail for the
    /// wayland backend, and is no longer relevant.
    ///
    /// Always return true.
    #[deprecated]
    fn is_ready(&self) -> bool;
}

impl WindowExt for Window {
    #[inline]
    fn get_xlib_window(&self) -> Option<raw::c_ulong> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xlib_window()),
            _ => None
        }
    }

    #[inline]
    fn get_xlib_display(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xlib_display()),
            _ => None
        }
    }

    fn get_xlib_screen_id(&self) -> Option<raw::c_int> {
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

    fn get_xcb_connection(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xcb_connection()),
            _ => None
        }
    }

    #[inline]
    fn get_wayland_surface(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.get_surface().c_ptr() as *mut _),
            _ => None
        }
    }

    #[inline]
    fn get_wayland_display(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.get_display().c_ptr() as *mut _),
            _ => None
        }
    }

    #[inline]
    fn is_ready(&self) -> bool {
        true
    }
}

/// Additional methods on `WindowBuilder` that are specific to Unix.
pub trait WindowBuilderExt {
    fn with_x11_visual<T>(self, visual_infos: *const T) -> WindowBuilder;
    fn with_x11_screen(self, screen_id: i32) -> WindowBuilder;

    /// Build window with resize increment hint. Only implemented on X11.
    fn with_resize_increments(self, width_inc: i32, height_inc: i32) -> WindowBuilder;
    /// Build window with base size hint. Only implemented on X11.
    fn with_base_size(self, base_width: i32, base_height: i32) -> WindowBuilder;
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

    #[inline]
    fn with_resize_increments(mut self, width_inc: i32, height_inc: i32) -> WindowBuilder {
        self.platform_specific.resize_increments = Some((width_inc, height_inc));
        self
    }

    #[inline]
    fn with_base_size(mut self, base_width: i32, base_height: i32) -> WindowBuilder {
        self.platform_specific.base_size = Some((base_width, base_height));
        self
    }
}

/// Additional methods on `MonitorId` that are specific to Linux.
pub trait MonitorIdExt {
    /// Returns the inner identifier of the monitor.
    fn native_id(&self) -> u32;
}

impl MonitorIdExt for MonitorId {
    #[inline]
    fn native_id(&self) -> u32 {
        self.inner.get_native_identifier()
    }
}
