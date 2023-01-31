use std::os::raw;

use crate::{
    event_loop::{EventLoopBuilder, EventLoopWindowTarget},
    monitor::MonitorHandle,
    window::{Window, WindowBuilder},
};

use crate::platform_impl::{
    ApplicationName, Backend, EventLoopWindowTarget as LinuxEventLoopWindowTarget,
    Window as LinuxWindow,
};

pub use crate::window::Theme;

/// Additional methods on [`EventLoopWindowTarget`] that are specific to Wayland.
pub trait EventLoopWindowTargetExtWayland {
    /// True if the [`EventLoopWindowTarget`] uses Wayland.
    fn is_wayland(&self) -> bool;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this
    /// [`EventLoopWindowTarget`].
    ///
    /// Returns `None` if the [`EventLoop`] doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the winit [`EventLoop`] is destroyed.
    ///
    /// [`EventLoop`]: crate::event_loop::EventLoop
    fn wayland_display(&self) -> Option<*mut raw::c_void>;
}

impl<T> EventLoopWindowTargetExtWayland for EventLoopWindowTarget<T> {
    #[inline]
    fn is_wayland(&self) -> bool {
        self.p.is_wayland()
    }

    #[inline]
    fn wayland_display(&self) -> Option<*mut raw::c_void> {
        match self.p {
            LinuxEventLoopWindowTarget::Wayland(ref p) => {
                Some(p.display().get_display_ptr() as *mut _)
            }
            #[cfg(x11_platform)]
            _ => None,
        }
    }
}

/// Additional methods on [`EventLoopBuilder`] that are specific to Wayland.
pub trait EventLoopBuilderExtWayland {
    /// Force using Wayland.
    fn with_wayland(&mut self) -> &mut Self;

    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;
}

impl<T> EventLoopBuilderExtWayland for EventLoopBuilder<T> {
    #[inline]
    fn with_wayland(&mut self) -> &mut Self {
        self.platform_specific.forced_backend = Some(Backend::Wayland);
        self
    }

    #[inline]
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.platform_specific.any_thread = any_thread;
        self
    }
}

/// Additional methods on [`Window`] that are specific to Wayland.
pub trait WindowExtWayland {
    /// Returns a pointer to the `wl_surface` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn wayland_surface(&self) -> Option<*mut raw::c_void>;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn wayland_display(&self) -> Option<*mut raw::c_void>;
}

impl WindowExtWayland for Window {
    #[inline]
    fn wayland_surface(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.surface().as_ref().c_ptr() as *mut _),
            #[cfg(x11_platform)]
            _ => None,
        }
    }

    #[inline]
    fn wayland_display(&self) -> Option<*mut raw::c_void> {
        match self.window {
            LinuxWindow::Wayland(ref w) => Some(w.display().get_display_ptr() as *mut _),
            #[cfg(x11_platform)]
            _ => None,
        }
    }
}

/// Additional methods on [`WindowBuilder`] that are specific to Wayland.
pub trait WindowBuilderExtWayland {
    /// Build window with the given name.
    ///
    /// The `general` name sets an application ID, which should match the `.desktop`
    /// file destributed with your program. The `instance` is a `no-op`.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    fn with_name(self, general: impl Into<String>, instance: impl Into<String>) -> Self;
}

impl WindowBuilderExtWayland for WindowBuilder {
    #[inline]
    fn with_name(mut self, general: impl Into<String>, instance: impl Into<String>) -> Self {
        self.platform_specific.name = Some(ApplicationName::new(general.into(), instance.into()));
        self
    }
}

/// Additional methods on `MonitorHandle` that are specific to Wayland.
pub trait MonitorHandleExtWayland {
    /// Returns the inner identifier of the monitor.
    fn native_id(&self) -> u32;
}

impl MonitorHandleExtWayland for MonitorHandle {
    #[inline]
    fn native_id(&self) -> u32 {
        self.inner.native_identifier()
    }
}
