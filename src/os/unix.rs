#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]

use std::os::raw;
use std::ptr;
use std::sync::Arc;

use sctk::window::{ButtonState, Theme};

use {
    EventsLoop,
    LogicalSize,
    MonitorId,
    Window,
    WindowBuilder,
};
use platform::{
    EventsLoop as LinuxEventsLoop,
    Window as LinuxWindow,
};
use platform::x11::XConnection;
use platform::x11::ffi::XVisualInfo;

// TODO: stupid hack so that glutin can do its work
#[doc(hidden)]
pub use platform::x11;

pub use platform::XNotSupported;
pub use platform::x11::util::WindowType as XWindowType;

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

    /// Returns a pointer to the `wl_display` object of wayland that is used by this `EventsLoop`.
    ///
    /// Returns `None` if the `EventsLoop` doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the glutin `EventsLoop` is destroyed.
    fn get_wayland_display(&self) -> Option<*mut raw::c_void>;
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
    #[doc(hidden)]
    fn get_xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        self.events_loop.x_connection().cloned()
    }

    #[inline]
    fn get_wayland_display(&self) -> Option<*mut raw::c_void> {
        match self.events_loop {
            LinuxEventsLoop::Wayland(ref e) => Some(e.get_display().c_ptr() as *mut _),
            _ => None
        }
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

    #[doc(hidden)]
    fn get_xlib_xconnection(&self) -> Option<Arc<XConnection>>;

    /// Set window urgency hint (`XUrgencyHint`). Only relevant on X.
    fn set_urgent(&self, is_urgent: bool);

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

    #[inline]
    fn get_xlib_screen_id(&self) -> Option<raw::c_int> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xlib_screen_id()),
            _ => None
        }
    }

    #[inline]
    #[doc(hidden)]
    fn get_xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xlib_xconnection()),
            _ => None
        }
    }

    #[inline]
    fn get_xcb_connection(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.get_xcb_connection()),
            _ => None
        }
    }

    #[inline]
    fn set_urgent(&self, is_urgent: bool) {
        if let LinuxWindow::X(ref w) = self.window {
            w.set_urgent(is_urgent);
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
pub trait WindowBuilderExt {
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
