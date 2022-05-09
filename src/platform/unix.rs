#![cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

use std::os::raw;
#[cfg(feature = "x11")]
use std::ptr;

#[cfg(feature = "x11")]
use std::sync::Arc;

use crate::{
    event_loop::{EventLoopBuilder, EventLoopWindowTarget},
    monitor::MonitorHandle,
    window::{Window, WindowBuilder},
};

#[cfg(feature = "x11")]
use crate::dpi::Size;
#[cfg(feature = "x11")]
use crate::platform_impl::x11::{ffi::XVisualInfo, XConnection};
use crate::platform_impl::{
    kms::MODE, ApplicationName, EventLoopWindowTarget as LinuxEventLoopWindowTarget,
    Window as LinuxWindow,
};

#[cfg(any(feature = "x11", feature = "wayland", feature = "kms"))]
pub use crate::platform_impl::Backend;

// TODO: stupid hack so that glutin can do its work
#[cfg(feature = "kms")]
pub use crate::platform_impl::kms::Card;
#[doc(hidden)]
#[cfg(feature = "x11")]
pub use crate::platform_impl::x11;
#[cfg(feature = "x11")]
pub use crate::platform_impl::{x11::util::WindowType as XWindowType, XNotSupported};
#[cfg(feature = "kms")]
use drm::control::*;

/// Additional methods on `EventLoopWindowTarget` that are specific to Unix.
pub trait EventLoopWindowTargetExtUnix {
    /// Find out what backend winit is currently using.
    fn unix_backend(&self) -> Backend;

    #[doc(hidden)]
    #[cfg(feature = "x11")]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>>;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this
    /// `EventLoopWindowTarget`.
    ///
    /// Returns `None` if the `EventLoop` doesn't use wayland.
    ///
    /// The pointer will become invalid when the winit `EventLoop` is destroyed.
    #[cfg(feature = "wayland")]
    fn wayland_display(&self) -> Option<*mut raw::c_void>;

    /// Returns the drm device of the event loop's fd
    ///
    /// Returns `None` if the `EventLoop` doesn't use drm.
    #[cfg(feature = "kms")]
    fn drm_device(&self) -> Option<&Card>;

    /// Returns the current crtc of the drm device
    ///
    /// Returns `None` if the `EventLoop` doesn't use drm.
    #[cfg(feature = "kms")]
    fn drm_crtc(&self) -> Option<&crtc::Info>;

    /// Returns the current connector of the drm device
    ///
    /// Returns `None` if the `EventLoop` doesn't use drm.
    #[cfg(feature = "kms")]
    fn drm_connector(&self) -> Option<&connector::Info>;

    /// Returns the current mode of the drm device
    ///
    /// Returns `None` if the `EventLoop` doesn't use drm.
    #[cfg(feature = "kms")]
    fn drm_mode(&self) -> Option<Mode>;

    /// Returns the primary plane of the drm device
    ///
    /// Returns `None` if the `EventLoop` doesn't use drm.
    #[cfg(feature = "kms")]
    fn drm_plane(&self) -> Option<plane::Handle>;
}

impl<T> EventLoopWindowTargetExtUnix for EventLoopWindowTarget<T> {
    #[inline]
    fn unix_backend(&self) -> Backend {
        match self.p {
            LinuxEventLoopWindowTarget::X(_) => Backend::X,
            LinuxEventLoopWindowTarget::Wayland(_) => Backend::Wayland,
            LinuxEventLoopWindowTarget::Kms(_) => Backend::Kms,
        }
    }

    #[inline]
    #[doc(hidden)]
    #[cfg(feature = "x11")]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        match self.p {
            LinuxEventLoopWindowTarget::X(ref e) => Some(e.x_connection().clone()),
            #[cfg(any(feature = "wayland", feature = "kms"))]
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
            #[cfg(any(feature = "x11", feature = "kms"))]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "kms")]
    fn drm_device(&self) -> Option<&Card> {
        match self.p {
            LinuxEventLoopWindowTarget::Kms(ref evlp) => Some(&evlp.device),
            #[cfg(any(feature = "x11", feature = "wayland"))]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "kms")]
    fn drm_crtc(&self) -> Option<&crtc::Info> {
        match self.p {
            LinuxEventLoopWindowTarget::Kms(ref window) => Some(&window.crtc),
            #[cfg(any(feature = "x11", feature = "wayland"))]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "kms")]
    fn drm_connector(&self) -> Option<&connector::Info> {
        match self.p {
            LinuxEventLoopWindowTarget::Kms(ref window) => Some(&window.connector),
            #[cfg(any(feature = "x11", feature = "wayland"))]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "kms")]
    fn drm_mode(&self) -> Option<drm::control::Mode> {
        match self.p {
            LinuxEventLoopWindowTarget::Kms(_) => MODE.lock().clone(),
            #[cfg(any(feature = "x11", feature = "wayland"))]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "kms")]
    fn drm_plane(&self) -> Option<drm::control::plane::Handle> {
        match self.p {
            LinuxEventLoopWindowTarget::Kms(ref window) => Some(window.plane),
            #[cfg(any(feature = "x11", feature = "wayland"))]
            _ => None,
        }
    }
}

/// Additional methods on [`EventLoopBuilder`] that are specific to Unix.
pub trait EventLoopBuilderExtUnix {
    /// Force using X11.
    #[cfg(feature = "x11")]
    fn with_x11(&mut self) -> &mut Self;

    /// Force using Wayland.
    #[cfg(feature = "wayland")]
    fn with_wayland(&mut self) -> &mut Self;

    /// Force using kms
    #[cfg(feature = "kms")]
    fn with_drm(&mut self) -> &mut Self;

    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;
}

impl<T> EventLoopBuilderExtUnix for EventLoopBuilder<T> {
    #[inline]
    #[cfg(feature = "x11")]
    fn with_x11(&mut self) -> &mut Self {
        self.platform_specific.forced_backend = Some(Backend::X);
        self
    }

    #[inline]
    #[cfg(feature = "wayland")]
    fn with_wayland(&mut self) -> &mut Self {
        self.platform_specific.forced_backend = Some(Backend::Wayland);
        self
    }

    #[inline]
    #[cfg(feature = "kms")]
    fn with_drm(&mut self) -> &mut Self {
        self.platform_specific.forced_backend = Some(Backend::Kms);
        self
    }

    #[inline]
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.platform_specific.any_thread = any_thread;
        self
    }
}

/// Additional methods on `Window` that are specific to Unix.
pub trait WindowExtUnix {
    /// Returns the ID of the `Window` xlib object that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib.
    #[cfg(feature = "x11")]
    fn xlib_window(&self) -> Option<raw::c_ulong>;

    /// Returns a pointer to the `Display` object of xlib that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib.
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
    /// Returns `None` if the window doesn't use xlib.
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    #[cfg(feature = "x11")]
    fn xcb_connection(&self) -> Option<*mut raw::c_void>;

    /// Returns a pointer to the `wl_surface` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland.
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    #[cfg(feature = "wayland")]
    fn wayland_surface(&self) -> Option<*mut raw::c_void>;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland.
    ///
    /// The pointer will become invalid when the glutin `Window` is destroyed.
    #[cfg(feature = "wayland")]
    fn wayland_display(&self) -> Option<*mut raw::c_void>;

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
            #[cfg(any(feature = "wayland", feature = "kms"))]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn xlib_display(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_display()),
            #[cfg(any(feature = "wayland", feature = "kms"))]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn xlib_screen_id(&self) -> Option<raw::c_int> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_screen_id()),
            #[cfg(any(feature = "wayland", feature = "kms"))]
            _ => None,
        }
    }

    #[inline]
    #[doc(hidden)]
    #[cfg(feature = "x11")]
    fn xlib_xconnection(&self) -> Option<Arc<XConnection>> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xlib_xconnection()),
            #[cfg(any(feature = "wayland", feature = "kms"))]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "x11")]
    fn xcb_connection(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::X(ref w) => Some(w.xcb_connection()),
            #[cfg(any(feature = "wayland", feature = "kms"))]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "wayland")]
    fn wayland_surface(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.surface().as_ref().c_ptr() as *mut _),
            #[cfg(any(feature = "x11", feature = "kms"))]
            _ => None,
        }
    }

    #[inline]
    #[cfg(feature = "wayland")]
    fn wayland_display(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.display().get_display_ptr() as *mut _),
            #[cfg(any(feature = "x11", feature = "kms"))]
            _ => None,
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

    /// Build window with the given `general` and `instance` names.
    ///
    /// On Wayland, the `general` name sets an application ID, which should match the `.desktop`
    /// file destributed with your program. The `instance` is a `no-op`.
    ///
    /// On X11, the `general` sets general class of `WM_CLASS(STRING)`, while `instance` set the
    /// instance part of it. The resulted property looks like `WM_CLASS(STRING) = "general", "instance"`.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    fn with_name(self, general: impl Into<String>, instance: impl Into<String>) -> Self;

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
    ///
    /// ```
    /// # use winit::dpi::{LogicalSize, PhysicalSize};
    /// # use winit::window::WindowBuilder;
    /// # use winit::platform::unix::WindowBuilderExtUnix;
    /// // Specify the size in logical dimensions like this:
    /// WindowBuilder::new().with_resize_increments(LogicalSize::new(400.0, 200.0));
    ///
    /// // Or specify the size in physical dimensions like this:
    /// WindowBuilder::new().with_resize_increments(PhysicalSize::new(400, 200));
    /// ```
    #[cfg(feature = "x11")]
    fn with_resize_increments<S: Into<Size>>(self, increments: S) -> Self;

    /// Build window with base size hint. Only implemented on X11.
    ///
    /// ```
    /// # use winit::dpi::{LogicalSize, PhysicalSize};
    /// # use winit::window::WindowBuilder;
    /// # use winit::platform::unix::WindowBuilderExtUnix;
    /// // Specify the size in logical dimensions like this:
    /// WindowBuilder::new().with_base_size(LogicalSize::new(400.0, 200.0));
    ///
    /// // Or specify the size in physical dimensions like this:
    /// WindowBuilder::new().with_base_size(PhysicalSize::new(400, 200));
    /// ```
    #[cfg(feature = "x11")]
    fn with_base_size<S: Into<Size>>(self, base_size: S) -> Self;
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
    fn with_name(mut self, general: impl Into<String>, instance: impl Into<String>) -> Self {
        self.platform_specific.name = Some(ApplicationName::new(general.into(), instance.into()));
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
