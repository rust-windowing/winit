use crate::{
    event_loop::{EventLoopBuilder, EventLoopWindowTarget},
    monitor::MonitorHandle,
    window::{Window, WindowBuilder},
};

use crate::dpi::Size;
use crate::platform_impl::{ApplicationName, Backend, XLIB_ERROR_HOOKS};

pub use crate::platform_impl::{x11::util::WindowType as XWindowType, XNotSupported};

/// The first argument in the provided hook will be the pointer to `XDisplay`
/// and the second one the pointer to [`XErrorEvent`]. The returned `bool` is an
/// indicator whether the error was handled by the callback.
///
/// [`XErrorEvent`]: https://linux.die.net/man/3/xerrorevent
pub type XlibErrorHook =
    Box<dyn Fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> bool + Send + Sync>;

/// A unique identifer for an X11 visual.
pub type XVisualID = u32;

/// A unique identifier for an X11 window.
pub type XWindow = u32;

/// Hook to winit's xlib error handling callback.
///
/// This method is provided as a safe way to handle the errors comming from X11
/// when using xlib in external crates, like glutin for GLX access. Trying to
/// handle errors by speculating with `XSetErrorHandler` is [`unsafe`].
///
/// **Be aware that your hook is always invoked and returning `true` from it will
/// prevent `winit` from getting the error itself. It's wise to always return
/// `false` if you're not initiated the `Sync`.**
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
}

impl<T> EventLoopWindowTargetExtX11 for EventLoopWindowTarget<T> {
    #[inline]
    fn is_x11(&self) -> bool {
        !self.p.is_wayland()
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
pub trait WindowExtX11 {}

impl WindowExtX11 for Window {}

/// Additional methods on [`WindowBuilder`] that are specific to X11.
pub trait WindowBuilderExtX11 {
    /// Create this window with a specific X11 visual.
    fn with_x11_visual(self, visual_id: XVisualID) -> Self;

    fn with_x11_screen(self, screen_id: i32) -> Self;

    /// Build window with the given `general` and `instance` names.
    ///
    /// The `general` sets general class of `WM_CLASS(STRING)`, while `instance` set the
    /// instance part of it. The resulted property looks like `WM_CLASS(STRING) = "instance", "general"`.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    fn with_name(self, general: impl Into<String>, instance: impl Into<String>) -> Self;

    /// Build window with override-redirect flag; defaults to false. Only relevant on X11.
    fn with_override_redirect(self, override_redirect: bool) -> Self;

    /// Build window with `_NET_WM_WINDOW_TYPE` hints; defaults to `Normal`. Only relevant on X11.
    fn with_x11_window_type(self, x11_window_type: Vec<XWindowType>) -> Self;

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

    /// Embed this window into another parent window.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use winit::window::WindowBuilder;
    /// use winit::platform::x11::{XWindow, WindowBuilderExtX11};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let event_loop = winit::event_loop::EventLoop::new().unwrap();
    /// let parent_window_id = std::env::args().nth(1).unwrap().parse::<XWindow>()?;
    /// let window = WindowBuilder::new()
    ///     .with_embed_parent_window(parent_window_id)
    ///     .build(&event_loop)?;
    /// # Ok(()) }
    /// ```
    fn with_embed_parent_window(self, parent_window_id: XWindow) -> Self;
}

impl WindowBuilderExtX11 for WindowBuilder {
    #[inline]
    fn with_x11_visual(mut self, visual_id: XVisualID) -> Self {
        self.platform_specific.x11.visual_id = Some(visual_id);
        self
    }

    #[inline]
    fn with_x11_screen(mut self, screen_id: i32) -> Self {
        self.platform_specific.x11.screen_id = Some(screen_id);
        self
    }

    #[inline]
    fn with_name(mut self, general: impl Into<String>, instance: impl Into<String>) -> Self {
        self.platform_specific.name = Some(ApplicationName::new(general.into(), instance.into()));
        self
    }

    #[inline]
    fn with_override_redirect(mut self, override_redirect: bool) -> Self {
        self.platform_specific.x11.override_redirect = override_redirect;
        self
    }

    #[inline]
    fn with_x11_window_type(mut self, x11_window_types: Vec<XWindowType>) -> Self {
        self.platform_specific.x11.x11_window_types = x11_window_types;
        self
    }

    #[inline]
    fn with_base_size<S: Into<Size>>(mut self, base_size: S) -> Self {
        self.platform_specific.x11.base_size = Some(base_size.into());
        self
    }

    #[inline]
    fn with_embed_parent_window(mut self, parent_window_id: XWindow) -> Self {
        self.platform_specific.x11.embed_window = Some(parent_window_id);
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
