//! # X11

use dpi::Size;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use winit_core::event_loop::ActiveEventLoop as CoreActiveEventLoop;
use winit_core::window::{ActivationToken, PlatformWindowAttributes, Window as CoreWindow};

pub use crate::event_loop::{ActiveEventLoop, EventLoop};
pub use crate::window::Window;

macro_rules! os_error {
    ($error:expr) => {{
        winit_core::error::OsError::new(line!(), file!(), $error)
    }};
}

mod activation;
mod atoms;
mod dnd;
mod event_loop;
mod event_processor;
pub mod ffi;
mod ime;
mod monitor;
mod util;
mod window;
mod xdisplay;
mod xsettings;

/// X window type. Maps directly to
/// [`_NET_WM_WINDOW_TYPE`](https://specifications.freedesktop.org/wm-spec/wm-spec-1.5.html).
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum WindowType {
    /// A desktop feature. This can include a single window containing desktop icons with the same
    /// dimensions as the screen, allowing the desktop environment to have full control of the
    /// desktop, without the need for proxying root window clicks.
    Desktop,
    /// A dock or panel feature. Typically a Window Manager would keep such windows on top of all
    /// other windows.
    Dock,
    /// Toolbar windows. "Torn off" from the main application.
    Toolbar,
    /// Pinnable menu windows. "Torn off" from the main application.
    Menu,
    /// A small persistent utility window, such as a palette or toolbox.
    Utility,
    /// The window is a splash screen displayed as an application is starting up.
    Splash,
    /// This is a dialog window.
    Dialog,
    /// A dropdown menu that usually appears when the user clicks on an item in a menu bar.
    /// This property is typically used on override-redirect windows.
    DropdownMenu,
    /// A popup menu that usually appears when the user right clicks on an object.
    /// This property is typically used on override-redirect windows.
    PopupMenu,
    /// A tooltip window. Usually used to show additional information when hovering over an object
    /// with the cursor. This property is typically used on override-redirect windows.
    Tooltip,
    /// The window is a notification.
    /// This property is typically used on override-redirect windows.
    Notification,
    /// This should be used on the windows that are popped up by combo boxes.
    /// This property is typically used on override-redirect windows.
    Combo,
    /// This indicates the window is being dragged.
    /// This property is typically used on override-redirect windows.
    Dnd,
    /// This is a normal, top-level window.
    #[default]
    Normal,
}

/// The first argument in the provided hook will be the pointer to `XDisplay`
/// and the second one the pointer to [`XErrorEvent`]. The returned `bool` is an
/// indicator whether the error was handled by the callback.
///
/// [`XErrorEvent`]: https://linux.die.net/man/3/xerrorevent
pub type XlibErrorHook =
    Box<dyn Fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> bool + Send + Sync>;

/// A unique identifier for an X11 visual.
pub type XVisualID = u32;

/// A unique identifier for an X11 window.
pub type XWindow = u32;

/// Hook to winit's xlib error handling callback.
///
/// This method is provided as a safe way to handle the errors coming from X11
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
    crate::event_loop::XLIB_ERROR_HOOKS.lock().unwrap().push(hook);
}

/// Additional methods on [`ActiveEventLoop`] that are specific to X11.
///
/// [`ActiveEventLoop`]: winit_core::event_loop::ActiveEventLoop
pub trait ActiveEventLoopExtX11 {
    /// True if the event loop uses X11.
    fn is_x11(&self) -> bool;
}

impl ActiveEventLoopExtX11 for dyn CoreActiveEventLoop + '_ {
    #[inline]
    fn is_x11(&self) -> bool {
        self.cast_ref::<ActiveEventLoop>().is_some()
    }
}

/// Additional methods on [`EventLoop`] that are specific to X11.
pub trait EventLoopExtX11 {
    /// True if the [`EventLoop`] uses X11.
    fn is_x11(&self) -> bool;
}

/// Additional methods when building event loop that are specific to X11.
pub trait EventLoopBuilderExtX11 {
    /// Force using X11.
    fn with_x11(&mut self) -> &mut Self;

    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;
}

/// Additional methods on [`Window`] that are specific to X11.
///
/// [`Window`]: crate::window::Window
pub trait WindowExtX11 {}

impl WindowExtX11 for dyn CoreWindow {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplicationName {
    pub(crate) general: String,
    pub(crate) instance: String,
}

#[derive(Clone, Debug)]
pub struct WindowAttributesX11 {
    pub(crate) name: Option<ApplicationName>,
    pub(crate) activation_token: Option<ActivationToken>,
    pub(crate) visual_id: Option<XVisualID>,
    pub(crate) screen_id: Option<i32>,
    pub(crate) base_size: Option<Size>,
    pub(crate) override_redirect: bool,
    pub(crate) x11_window_types: Vec<WindowType>,

    /// The parent window to embed this window into.
    pub(crate) embed_window: Option<XWindow>,
}

impl Default for WindowAttributesX11 {
    fn default() -> Self {
        Self {
            name: None,
            activation_token: None,
            visual_id: None,
            screen_id: None,
            base_size: None,
            override_redirect: false,
            x11_window_types: vec![WindowType::Normal],
            embed_window: None,
        }
    }
}

impl WindowAttributesX11 {
    /// Create this window with a specific X11 visual.
    pub fn with_x11_visual(mut self, visual_id: XVisualID) -> Self {
        self.visual_id = Some(visual_id);
        self
    }

    pub fn with_x11_screen(mut self, screen_id: i32) -> Self {
        self.screen_id = Some(screen_id);
        self
    }

    /// Build window with the given `general` and `instance` names.
    ///
    /// The `general` sets general class of `WM_CLASS(STRING)`, while `instance` set the
    /// instance part of it. The resulted property looks like `WM_CLASS(STRING) = "instance",
    /// "general"`.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    pub fn with_name(mut self, general: impl Into<String>, instance: impl Into<String>) -> Self {
        self.name = Some(ApplicationName { general: general.into(), instance: instance.into() });
        self
    }

    /// Build window with override-redirect flag; defaults to false.
    pub fn with_override_redirect(mut self, override_redirect: bool) -> Self {
        self.override_redirect = override_redirect;
        self
    }

    /// Build window with `_NET_WM_WINDOW_TYPE` hints; defaults to `Normal`.
    pub fn with_x11_window_type(mut self, x11_window_types: Vec<WindowType>) -> Self {
        self.x11_window_types = x11_window_types;
        self
    }

    /// Build window with base size hint.
    ///
    /// ```
    /// # use winit::dpi::{LogicalSize, PhysicalSize};
    /// # use winit::window::{Window, WindowAttributes};
    /// # use winit::platform::x11::WindowAttributesX11;
    /// // Specify the size in logical dimensions like this:
    /// WindowAttributesX11::default().with_base_size(LogicalSize::new(400.0, 200.0));
    ///
    /// // Or specify the size in physical dimensions like this:
    /// WindowAttributesX11::default().with_base_size(PhysicalSize::new(400, 200));
    /// ```
    pub fn with_base_size<S: Into<Size>>(mut self, base_size: S) -> Self {
        self.base_size = Some(base_size.into());
        self
    }

    /// Embed this window into another parent window.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use winit::window::{Window, WindowAttributes};
    /// use winit::event_loop::ActiveEventLoop;
    /// use winit::platform::x11::{XWindow, WindowAttributesX11};
    /// # fn create_window(event_loop: &dyn ActiveEventLoop) -> Result<(), Box<dyn std::error::Error>> {
    /// let parent_window_id = std::env::args().nth(1).unwrap().parse::<XWindow>()?;
    /// let window_x11_attributes = WindowAttributesX11::default().with_embed_parent_window(parent_window_id);
    /// let window_attributes = WindowAttributes::default().with_platform_attributes(Box::new(window_x11_attributes));
    /// let window = event_loop.create_window(window_attributes)?;
    /// # Ok(()) }
    /// ```
    pub fn with_embed_parent_window(mut self, parent_window_id: XWindow) -> Self {
        self.embed_window = Some(parent_window_id);
        self
    }

    #[inline]
    pub fn with_activation_token(mut self, token: ActivationToken) -> Self {
        self.activation_token = Some(token);
        self
    }
}

impl PlatformWindowAttributes for WindowAttributesX11 {
    fn box_clone(&self) -> Box<dyn PlatformWindowAttributes> {
        Box::from(self.clone())
    }
}
