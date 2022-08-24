//! The [`Window`] struct and associated types.
use std::fmt;

use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError},
    event_loop::EventLoopWindowTarget,
    monitor::{MonitorHandle, VideoMode},
    platform_impl,
};

pub use crate::icon::{BadIcon, Icon};

/// Represents a window.
///
/// # Example
///
/// ```no_run
/// use winit::{
///     event::{Event, WindowEvent},
///     event_loop::EventLoop,
///     window::Window,
/// };
///
/// let mut event_loop = EventLoop::new();
/// let window = Window::new(&event_loop).unwrap();
///
/// event_loop.run(move |event, _, control_flow| {
///     control_flow.set_wait();
///
///     match event {
///         Event::WindowEvent {
///             event: WindowEvent::CloseRequested,
///             ..
///         } => control_flow.set_exit(),
///         _ => (),
///     }
/// });
/// ```
pub struct Window {
    pub(crate) window: platform_impl::Window,
}

impl fmt::Debug for Window {
    fn fmt(&self, fmtr: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmtr.pad("Window { .. }")
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // If the window is in exclusive fullscreen, we must restore the desktop
        // video mode (generally this would be done on application exit, but
        // closing the window doesn't necessarily always mean application exit,
        // such as when there are multiple windows)
        if let Some(Fullscreen::Exclusive(_)) = self.fullscreen() {
            self.set_fullscreen(None);
        }
    }
}

/// Identifier of a window. Unique for each window.
///
/// Can be obtained with [`window.id()`](`Window::id`).
///
/// Whenever you receive an event specific to a window, this event contains a `WindowId` which you
/// can then compare to the ids of your windows.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub(crate) platform_impl::WindowId);

impl WindowId {
    /// Returns a dummy id, useful for unit testing.
    ///
    /// # Safety
    ///
    /// The only guarantee made about the return value of this function is that
    /// it will always be equal to itself and to future values returned by this function.
    /// No other guarantees are made. This may be equal to a real [`WindowId`].
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub const unsafe fn dummy() -> Self {
        WindowId(platform_impl::WindowId::dummy())
    }
}

impl From<WindowId> for u64 {
    fn from(window_id: WindowId) -> Self {
        window_id.0.into()
    }
}

impl From<u64> for WindowId {
    fn from(raw_id: u64) -> Self {
        Self(raw_id.into())
    }
}

/// Object that allows building windows.
#[derive(Clone, Default)]
#[must_use]
pub struct WindowBuilder {
    /// The attributes to use to create the window.
    pub(crate) window: WindowAttributes,

    // Platform-specific configuration.
    pub(crate) platform_specific: platform_impl::PlatformSpecificWindowBuilderAttributes,
}

impl fmt::Debug for WindowBuilder {
    fn fmt(&self, fmtr: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmtr.debug_struct("WindowBuilder")
            .field("window", &self.window)
            .finish()
    }
}

/// Attributes to use when creating a window.
#[derive(Debug, Clone)]
pub(crate) struct WindowAttributes {
    pub inner_size: Option<Size>,
    pub min_inner_size: Option<Size>,
    pub max_inner_size: Option<Size>,
    pub position: Option<Position>,
    pub resizable: bool,
    pub title: String,
    pub fullscreen: Option<Fullscreen>,
    pub maximized: bool,
    pub visible: bool,
    pub transparent: bool,
    pub decorations: bool,
    pub always_on_top: bool,
    pub window_icon: Option<Icon>,
}

impl Default for WindowAttributes {
    #[inline]
    fn default() -> WindowAttributes {
        WindowAttributes {
            inner_size: None,
            min_inner_size: None,
            max_inner_size: None,
            position: None,
            resizable: true,
            title: "winit window".to_owned(),
            maximized: false,
            fullscreen: None,
            visible: true,
            transparent: false,
            decorations: true,
            always_on_top: false,
            window_icon: None,
        }
    }
}

impl WindowBuilder {
    /// Initializes a new builder with default values.
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    /// Requests the window to be of specific dimensions.
    ///
    /// If this is not set, some platform-specific dimensions will be used.
    ///
    /// See [`Window::set_inner_size`] for details.
    #[inline]
    pub fn with_inner_size<S: Into<Size>>(mut self, size: S) -> Self {
        self.window.inner_size = Some(size.into());
        self
    }

    /// Sets the minimum dimensions a window can have.
    ///
    /// If this is not set, the window will have no minimum dimensions (aside
    /// from reserved).
    ///
    /// See [`Window::set_min_inner_size`] for details.
    #[inline]
    pub fn with_min_inner_size<S: Into<Size>>(mut self, min_size: S) -> Self {
        self.window.min_inner_size = Some(min_size.into());
        self
    }

    /// Sets the maximum dimensions a window can have.
    ///
    /// If this is not set, the window will have no maximum or will be set to
    /// the primary monitor's dimensions by the platform.
    ///
    /// See [`Window::set_max_inner_size`] for details.
    #[inline]
    pub fn with_max_inner_size<S: Into<Size>>(mut self, max_size: S) -> Self {
        self.window.max_inner_size = Some(max_size.into());
        self
    }

    /// Sets a desired initial position for the window.
    ///
    /// If this is not set, some platform-specific position will be chosen.
    ///
    /// See [`Window::set_outer_position`] for details.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** The top left corner position of the window content, the
    ///   window's "inner" position. The window title bar will be placed above
    ///   it. The window will be positioned such that it fits on screen,
    ///   maintaining set `inner_size` if any.
    ///   If you need to precisely position the top left corner of the whole
    ///   window you have to use [`Window::set_outer_position`] after creating
    ///   the window.
    /// - **Windows:** The top left corner position of the window title bar,
    ///   the window's "outer" position.
    ///   There may be a small gap between this position and the window due to
    ///   the specifics of the Window Manager.
    /// - **X11:** The top left corner of the window, the window's "outer"
    ///   position.
    /// - **Others:** Ignored.
    #[inline]
    pub fn with_position<P: Into<Position>>(mut self, position: P) -> Self {
        self.window.position = Some(position.into());
        self
    }

    /// Sets whether the window is resizable or not.
    ///
    /// The default is `true`.
    ///
    /// See [`Window::set_resizable`] for details.
    #[inline]
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.window.resizable = resizable;
        self
    }

    /// Sets the initial title of the window in the title bar.
    ///
    /// The default is `"winit window"`.
    ///
    /// See [`Window::set_title`] for details.
    #[inline]
    pub fn with_title<T: Into<String>>(mut self, title: T) -> Self {
        self.window.title = title.into();
        self
    }

    /// Sets whether the window should be put into fullscreen upon creation.
    ///
    /// The default is `None`.
    ///
    /// See [`Window::set_fullscreen`] for details.
    #[inline]
    pub fn with_fullscreen(mut self, fullscreen: Option<Fullscreen>) -> Self {
        self.window.fullscreen = fullscreen;
        self
    }

    /// Request that the window is maximized upon creation.
    ///
    /// The default is `false`.
    ///
    /// See [`Window::set_maximized`] for details.
    #[inline]
    pub fn with_maximized(mut self, maximized: bool) -> Self {
        self.window.maximized = maximized;
        self
    }

    /// Sets whether the window will be initially visible or hidden.
    ///
    /// The default is to show the window.
    ///
    /// See [`Window::set_visible`] for details.
    #[inline]
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.window.visible = visible;
        self
    }

    /// Sets whether the background of the window should be transparent.
    ///
    /// If this is `true`, writing colors with alpha values different than
    /// `1.0` will produce a transparent window.
    ///
    /// The default is `false`.
    #[inline]
    pub fn with_transparent(mut self, transparent: bool) -> Self {
        self.window.transparent = transparent;
        self
    }

    /// Get whether the window will support transparency.
    #[inline]
    pub fn transparent(&self) -> bool {
        self.window.transparent
    }

    /// Sets whether the window should have a border, a title bar, etc.
    ///
    /// The default is `true`.
    ///
    /// See [`Window::set_decorations`] for details.
    #[inline]
    pub fn with_decorations(mut self, decorations: bool) -> Self {
        self.window.decorations = decorations;
        self
    }

    /// Sets whether or not the window will always be on top of other windows.
    ///
    /// The default is `false`.
    ///
    /// See [`Window::set_always_on_top`] for details.
    #[inline]
    pub fn with_always_on_top(mut self, always_on_top: bool) -> Self {
        self.window.always_on_top = always_on_top;
        self
    }

    /// Sets the window icon.
    ///
    /// The default is `None`.
    ///
    /// See [`Window::set_window_icon`] for details.
    #[inline]
    pub fn with_window_icon(mut self, window_icon: Option<Icon>) -> Self {
        self.window.window_icon = window_icon;
        self
    }

    /// Builds the window.
    ///
    /// Possible causes of error include denied permission, incompatible system, and lack of memory.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** The window is created but not inserted into the web page automatically. Please
    ///   see the web platform module for more information.
    #[inline]
    pub fn build<T: 'static>(
        self,
        window_target: &EventLoopWindowTarget<T>,
    ) -> Result<Window, OsError> {
        platform_impl::Window::new(&window_target.p, self.window, self.platform_specific).map(
            |window| {
                window.request_redraw();
                Window { window }
            },
        )
    }
}

/// Base Window functions.
impl Window {
    /// Creates a new Window for platforms where this is appropriate.
    ///
    /// This function is equivalent to [`WindowBuilder::new().build(event_loop)`].
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** The window is created but not inserted into the web page automatically. Please
    ///   see the web platform module for more information.
    ///
    /// [`WindowBuilder::new().build(event_loop)`]: WindowBuilder::build
    #[inline]
    pub fn new<T: 'static>(event_loop: &EventLoopWindowTarget<T>) -> Result<Window, OsError> {
        let builder = WindowBuilder::new();
        builder.build(event_loop)
    }

    /// Returns an identifier unique to the window.
    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.window.id())
    }

    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and vice versa.
    ///
    /// See the [`dpi`](crate::dpi) module for more information.
    ///
    /// Note that this value can change depending on user action (for example if the window is
    /// moved to another screen); as such, tracking [`WindowEvent::ScaleFactorChanged`] events is
    /// the most robust way to track the DPI you need to use to draw.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** This respects Xft.dpi, and can be overridden using the `WINIT_X11_SCALE_FACTOR` environment variable.
    /// - **Android:** Always returns 1.0.
    /// - **iOS:** Can only be called on the main thread. Returns the underlying `UIView`'s
    ///   [`contentScaleFactor`].
    ///
    /// [`WindowEvent::ScaleFactorChanged`]: crate::event::WindowEvent::ScaleFactorChanged
    /// [`contentScaleFactor`]: https://developer.apple.com/documentation/uikit/uiview/1622657-contentscalefactor?language=objc
    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    /// Emits a [`Event::RedrawRequested`] event in the associated event loop after all OS
    /// events have been processed by the event loop.
    ///
    /// This is the **strongly encouraged** method of redrawing windows, as it can integrate with
    /// OS-requested redraws (e.g. when a window gets resized).
    ///
    /// This function can cause `RedrawRequested` events to be emitted after [`Event::MainEventsCleared`]
    /// but before `Event::NewEvents` if called in the following circumstances:
    /// * While processing `MainEventsCleared`.
    /// * While processing a `RedrawRequested` event that was sent during `MainEventsCleared` or any
    ///   directly subsequent `RedrawRequested` event.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread.
    /// - **Android:** Subsequent calls after `MainEventsCleared` are not handled.
    ///
    /// [`Event::RedrawRequested`]: crate::event::Event::RedrawRequested
    /// [`Event::MainEventsCleared`]: crate::event::Event::MainEventsCleared
    #[inline]
    pub fn request_redraw(&self) {
        self.window.request_redraw()
    }
}

/// Position and size functions.
impl Window {
    /// Returns the position of the top-left hand corner of the window's client area relative to the
    /// top-left hand corner of the desktop.
    ///
    /// The same conditions that apply to [`Window::outer_position`] apply to this method.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Returns the top left coordinates of the
    ///   window's [safe area] in the screen space coordinate system.
    /// - **Web:** Returns the top-left coordinates relative to the viewport. _Note: this returns the
    ///    same value as [`Window::outer_position`]._
    /// - **Android / Wayland:** Always returns [`NotSupportedError`].
    ///
    /// [safe area]: https://developer.apple.com/documentation/uikit/uiview/2891103-safeareainsets?language=objc
    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        self.window.inner_position()
    }

    /// Returns the position of the top-left hand corner of the window relative to the
    /// top-left hand corner of the desktop.
    ///
    /// Note that the top-left hand corner of the desktop is not necessarily the same as
    /// the screen. If the user uses a desktop with multiple monitors, the top-left hand corner
    /// of the desktop is the top-left hand corner of the monitor at the top-left of the desktop.
    ///
    /// The coordinates can be negative if the top-left hand corner of the window is outside
    /// of the visible screen region.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Returns the top left coordinates of the
    ///   window in the screen space coordinate system.
    /// - **Web:** Returns the top-left coordinates relative to the viewport.
    /// - **Android / Wayland:** Always returns [`NotSupportedError`].
    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        self.window.outer_position()
    }

    /// Modifies the position of the window.
    ///
    /// See [`Window::outer_position`] for more information about the coordinates.
    /// This automatically un-maximizes the window if it's maximized.
    ///
    /// ```no_run
    /// # use winit::dpi::{LogicalPosition, PhysicalPosition};
    /// # use winit::event_loop::EventLoop;
    /// # use winit::window::Window;
    /// # let mut event_loop = EventLoop::new();
    /// # let window = Window::new(&event_loop).unwrap();
    /// // Specify the position in logical dimensions like this:
    /// window.set_outer_position(LogicalPosition::new(400.0, 200.0));
    ///
    /// // Or specify the position in physical dimensions like this:
    /// window.set_outer_position(PhysicalPosition::new(400, 200));
    /// ```
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Sets the top left coordinates of the
    ///   window in the screen space coordinate system.
    /// - **Web:** Sets the top-left coordinates relative to the viewport.
    /// - **Android / Wayland:** Unsupported.
    #[inline]
    pub fn set_outer_position<P: Into<Position>>(&self, position: P) {
        self.window.set_outer_position(position.into())
    }

    /// Returns the physical size of the window's client area.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Returns the `PhysicalSize` of the window's
    ///   [safe area] in screen space coordinates.
    /// - **Web:** Returns the size of the canvas element.
    ///
    /// [safe area]: https://developer.apple.com/documentation/uikit/uiview/2891103-safeareainsets?language=objc
    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    /// Modifies the inner size of the window.
    ///
    /// See [`Window::inner_size`] for more information about the values.
    /// This automatically un-maximizes the window if it's maximized.
    ///
    /// ```no_run
    /// # use winit::dpi::{LogicalSize, PhysicalSize};
    /// # use winit::event_loop::EventLoop;
    /// # use winit::window::Window;
    /// # let mut event_loop = EventLoop::new();
    /// # let window = Window::new(&event_loop).unwrap();
    /// // Specify the size in logical dimensions like this:
    /// window.set_inner_size(LogicalSize::new(400.0, 200.0));
    ///
    /// // Or specify the size in physical dimensions like this:
    /// window.set_inner_size(PhysicalSize::new(400, 200));
    /// ```
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android:** Unsupported.
    /// - **Web:** Sets the size of the canvas element.
    #[inline]
    pub fn set_inner_size<S: Into<Size>>(&self, size: S) {
        self.window.set_inner_size(size.into())
    }

    /// Returns the physical size of the entire window.
    ///
    /// These dimensions include the title bar and borders. If you don't want that (and you usually don't),
    /// use [`Window::inner_size`] instead.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Returns the [`PhysicalSize`] of the window in
    ///   screen space coordinates.
    /// - **Web:** Returns the size of the canvas element. _Note: this returns the same value as
    ///   [`Window::inner_size`]._
    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        self.window.outer_size()
    }

    /// Sets a minimum dimension size for the window.
    ///
    /// ```no_run
    /// # use winit::dpi::{LogicalSize, PhysicalSize};
    /// # use winit::event_loop::EventLoop;
    /// # use winit::window::Window;
    /// # let mut event_loop = EventLoop::new();
    /// # let window = Window::new(&event_loop).unwrap();
    /// // Specify the size in logical dimensions like this:
    /// window.set_min_inner_size(Some(LogicalSize::new(400.0, 200.0)));
    ///
    /// // Or specify the size in physical dimensions like this:
    /// window.set_min_inner_size(Some(PhysicalSize::new(400, 200)));
    /// ```
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web:** Unsupported.
    #[inline]
    pub fn set_min_inner_size<S: Into<Size>>(&self, min_size: Option<S>) {
        self.window.set_min_inner_size(min_size.map(|s| s.into()))
    }

    /// Sets a maximum dimension size for the window.
    ///
    /// ```no_run
    /// # use winit::dpi::{LogicalSize, PhysicalSize};
    /// # use winit::event_loop::EventLoop;
    /// # use winit::window::Window;
    /// # let mut event_loop = EventLoop::new();
    /// # let window = Window::new(&event_loop).unwrap();
    /// // Specify the size in logical dimensions like this:
    /// window.set_max_inner_size(Some(LogicalSize::new(400.0, 200.0)));
    ///
    /// // Or specify the size in physical dimensions like this:
    /// window.set_max_inner_size(Some(PhysicalSize::new(400, 200)));
    /// ```
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web:** Unsupported.
    #[inline]
    pub fn set_max_inner_size<S: Into<Size>>(&self, max_size: Option<S>) {
        self.window.set_max_inner_size(max_size.map(|s| s.into()))
    }
}

/// Misc. attribute functions.
impl Window {
    /// Modifies the title of the window.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android:** Unsupported.
    #[inline]
    pub fn set_title(&self, title: &str) {
        self.window.set_title(title)
    }

    /// Modifies the window's visibility.
    ///
    /// If `false`, this will hide the window. If `true`, this will show the window.
    ///
    /// ## Platform-specific
    ///
    /// - **Android / Wayland / Web:** Unsupported.
    /// - **iOS:** Can only be called on the main thread.
    #[inline]
    pub fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible)
    }

    /// Gets the window's current visibility state.
    ///
    /// `None` means it couldn't be determined, so it is not recommended to use this to drive your rendering backend.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Not implemented.
    /// - **Wayland / iOS / Android / Web:** Unsupported.
    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        self.window.is_visible()
    }

    /// Sets whether the window is resizable or not.
    ///
    /// Note that making the window unresizable doesn't exempt you from handling [`WindowEvent::Resized`], as that
    /// event can still be triggered by DPI scaling, entering fullscreen mode, etc. Also, the
    /// window could still be resized by calling [`Window::set_inner_size`].
    ///
    /// ## Platform-specific
    ///
    /// This only has an effect on desktop platforms.
    ///
    /// - **X11:** Due to a bug in XFCE, this has no effect on Xfwm.
    /// - **iOS / Android / Web:** Unsupported.
    ///
    /// [`WindowEvent::Resized`]: crate::event::WindowEvent::Resized
    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        self.window.set_resizable(resizable)
    }

    /// Gets the window's current resizable state.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Not implemented.
    /// - **iOS / Android / Web:** Unsupported.
    #[inline]
    pub fn is_resizable(&self) -> bool {
        self.window.is_resizable()
    }

    /// Sets the window to minimized or back
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web:** Unsupported.
    /// - **Wayland:** Un-minimize is unsupported.
    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        self.window.set_minimized(minimized);
    }

    /// Sets the window to maximized or back.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web:** Unsupported.
    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        self.window.set_maximized(maximized)
    }

    /// Gets the window's current maximized state.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web:** Unsupported.
    #[inline]
    pub fn is_maximized(&self) -> bool {
        self.window.is_maximized()
    }

    /// Sets the window to fullscreen or back.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** [`Fullscreen::Exclusive`] provides true exclusive mode with a
    ///   video mode change. *Caveat!* macOS doesn't provide task switching (or
    ///   spaces!) while in exclusive fullscreen mode. This mode should be used
    ///   when a video mode change is desired, but for a better user experience,
    ///   borderless fullscreen might be preferred.
    ///
    ///   [`Fullscreen::Borderless`] provides a borderless fullscreen window on a
    ///   separate space. This is the idiomatic way for fullscreen games to work
    ///   on macOS. See `WindowExtMacOs::set_simple_fullscreen` if
    ///   separate spaces are not preferred.
    ///
    ///   The dock and the menu bar are disabled in exclusive fullscreen mode.
    /// - **iOS:** Can only be called on the main thread.
    /// - **Wayland:** Does not support exclusive fullscreen mode and will no-op a request.
    /// - **Windows:** Screen saver is disabled in fullscreen mode.
    /// - **Android:** Unsupported.
    #[inline]
    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        self.window.set_fullscreen(fullscreen)
    }

    /// Gets the window's current fullscreen state.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread.
    /// - **Android:** Will always return `None`.
    /// - **Wayland:** Can return `Borderless(None)` when there are no monitors.
    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        self.window.fullscreen()
    }

    /// Turn window decorations on or off.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web:** Unsupported.
    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        self.window.set_decorations(decorations)
    }

    /// Gets the window's current decorations state.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Not implemented.
    /// - **iOS / Android / Web:** Unsupported.
    #[inline]
    pub fn is_decorated(&self) -> bool {
        self.window.is_decorated()
    }

    /// Change whether or not the window will always be on top of other windows.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web / Wayland:** Unsupported.
    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        self.window.set_always_on_top(always_on_top)
    }

    /// Sets the window icon.
    ///
    /// On Windows and X11, this is typically the small icon in the top-left
    /// corner of the titlebar.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web / Wayland / macOS:** Unsupported.
    ///
    /// - **Windows:** Sets `ICON_SMALL`. The base size for a window icon is 16x16, but it's
    ///   recommended to account for screen scaling and pick a multiple of that, i.e. 32x32.
    ///
    /// - **X11:** Has no universal guidelines for icon sizes, so you're at the whims of the WM. That
    ///   said, it's usually in the same ballpark as on Windows.
    #[inline]
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        self.window.set_window_icon(window_icon)
    }

    /// Sets location of IME candidate box in client area coordinates relative to the top left.
    ///
    /// This is the window / popup / overlay that allows you to select the desired characters.
    /// The look of this box may differ between input devices, even on the same platform.
    ///
    /// (Apple's official term is "candidate window", see their [chinese] and [japanese] guides).
    ///
    /// ## Example
    ///
    /// ```no_run
    /// # use winit::dpi::{LogicalPosition, PhysicalPosition};
    /// # use winit::event_loop::EventLoop;
    /// # use winit::window::Window;
    /// # let mut event_loop = EventLoop::new();
    /// # let window = Window::new(&event_loop).unwrap();
    /// // Specify the position in logical dimensions like this:
    /// window.set_ime_position(LogicalPosition::new(400.0, 200.0));
    ///
    /// // Or specify the position in physical dimensions like this:
    /// window.set_ime_position(PhysicalPosition::new(400, 200));
    /// ```
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web:** Unsupported.
    ///
    /// [chinese]: https://support.apple.com/guide/chinese-input-method/use-the-candidate-window-cim12992/104/mac/12.0
    /// [japanese]: https://support.apple.com/guide/japanese-input-method/use-the-candidate-window-jpim10262/6.3/mac/12.0
    #[inline]
    pub fn set_ime_position<P: Into<Position>>(&self, position: P) {
        self.window.set_ime_position(position.into())
    }

    /// Sets whether the window should get IME events
    ///
    /// When IME is allowed, the window will receive [`Ime`] events, and during the
    /// preedit phase the window will NOT get [`KeyboardInput`] or
    /// [`ReceivedCharacter`] events. The window should allow IME while it is
    /// expecting text input.
    ///
    /// When IME is not allowed, the window won't receive [`Ime`] events, and will
    /// receive [`KeyboardInput`] events for every keypress instead. Without
    /// allowing IME, the window will also get [`ReceivedCharacter`] events for
    /// certain keyboard input. Not allowing IME is useful for games for example.
    ///
    /// IME is **not** allowed by default.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** IME must be enabled to receive text-input where dead-key sequences are combined.
    /// - **iOS / Android / Web:** Unsupported.
    ///
    /// [`Ime`]: crate::event::WindowEvent::Ime
    /// [`KeyboardInput`]: crate::event::WindowEvent::KeyboardInput
    /// [`ReceivedCharacter`]: crate::event::WindowEvent::ReceivedCharacter
    #[inline]
    pub fn set_ime_allowed(&self, allowed: bool) {
        self.window.set_ime_allowed(allowed);
    }

    /// Brings the window to the front and sets input focus. Has no effect if the window is
    /// already in focus, minimized, or not visible.
    ///
    /// This method steals input focus from other applications. Do not use this method unless
    /// you are certain that's what the user wants. Focus stealing can cause an extremely disruptive
    /// user experience.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web / Wayland:** Unsupported.
    #[inline]
    pub fn focus_window(&self) {
        self.window.focus_window()
    }

    /// Requests user attention to the window, this has no effect if the application
    /// is already focused. How requesting for user attention manifests is platform dependent,
    /// see [`UserAttentionType`] for details.
    ///
    /// Providing `None` will unset the request for user attention. Unsetting the request for
    /// user attention might not be done automatically by the WM when the window receives input.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web:** Unsupported.
    /// - **macOS:** `None` has no effect.
    /// - **X11:** Requests for user attention must be manually cleared.
    /// - **Wayland:** Requires `xdg_activation_v1` protocol, `None` has no effect.
    #[inline]
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        self.window.request_user_attention(request_type)
    }
}

/// Cursor functions.
impl Window {
    /// Modifies the cursor icon of the window.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android:** Unsupported.
    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        self.window.set_cursor_icon(cursor);
    }

    /// Changes the position of the cursor in window coordinates.
    ///
    /// ```no_run
    /// # use winit::dpi::{LogicalPosition, PhysicalPosition};
    /// # use winit::event_loop::EventLoop;
    /// # use winit::window::Window;
    /// # let mut event_loop = EventLoop::new();
    /// # let window = Window::new(&event_loop).unwrap();
    /// // Specify the position in logical dimensions like this:
    /// window.set_cursor_position(LogicalPosition::new(400.0, 200.0));
    ///
    /// // Or specify the position in physical dimensions like this:
    /// window.set_cursor_position(PhysicalPosition::new(400, 200));
    /// ```
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web / Wayland:** Always returns an [`ExternalError::NotSupported`].
    #[inline]
    pub fn set_cursor_position<P: Into<Position>>(&self, position: P) -> Result<(), ExternalError> {
        self.window.set_cursor_position(position.into())
    }

    /// Set grabbing [mode]([`CursorGrabMode`]) on the cursor preventing it from leaving the window.
    ///
    /// # Example
    ///
    /// First try confining the cursor, and if that fails, try locking it instead.
    ///
    /// ```no_run
    /// # use winit::event_loop::EventLoop;
    /// # use winit::window::{CursorGrabMode, Window};
    /// # let mut event_loop = EventLoop::new();
    /// # let window = Window::new(&event_loop).unwrap();
    /// window.set_cursor_grab(CursorGrabMode::Confined)
    ///             .or_else(|_e| window.set_cursor_grab(CursorGrabMode::Locked))
    ///             .unwrap();
    /// ```
    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        self.window.set_cursor_grab(mode)
    }

    /// Modifies the cursor's visibility.
    ///
    /// If `false`, this will hide the cursor. If `true`, this will show the cursor.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows:** The cursor is only hidden within the confines of the window.
    /// - **X11:** The cursor is only hidden within the confines of the window.
    /// - **Wayland:** The cursor is only hidden within the confines of the window.
    /// - **macOS:** The cursor is hidden as long as the window has input focus, even if the cursor is
    ///   outside of the window.
    /// - **iOS / Android:** Unsupported.
    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        self.window.set_cursor_visible(visible)
    }

    /// Moves the window with the left mouse button until the button is released.
    ///
    /// There's no guarantee that this will work unless the left mouse button was pressed
    /// immediately before this function is called.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Un-grabs the cursor.
    /// - **Wayland:** Requires the cursor to be inside the window to be dragged.
    /// - **macOS:** May prevent the button release event to be triggered.
    /// - **iOS / Android / Web:** Always returns an [`ExternalError::NotSupported`].
    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        self.window.drag_window()
    }

    /// Modifies whether the window catches cursor events.
    ///
    /// If `true`, the window will catch the cursor events. If `false`, events are passed through
    /// the window such that any other window behind it receives them. By default hittest is enabled.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web / X11:** Always returns an [`ExternalError::NotSupported`].
    #[inline]
    pub fn set_cursor_hittest(&self, hittest: bool) -> Result<(), ExternalError> {
        self.window.set_cursor_hittest(hittest)
    }
}

/// Monitor info functions.
impl Window {
    /// Returns the monitor on which the window currently resides.
    ///
    /// Returns `None` if current monitor can't be detected.
    ///
    /// ## Platform-specific
    ///
    /// **iOS:** Can only be called on the main thread.
    #[inline]
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        self.window.current_monitor()
    }

    /// Returns the list of all the monitors available on the system.
    ///
    /// This is the same as [`EventLoopWindowTarget::available_monitors`], and is provided for convenience.
    ///
    /// ## Platform-specific
    ///
    /// **iOS:** Can only be called on the main thread.
    ///
    /// [`EventLoopWindowTarget::available_monitors`]: crate::event_loop::EventLoopWindowTarget::available_monitors
    #[inline]
    pub fn available_monitors(&self) -> impl Iterator<Item = MonitorHandle> {
        self.window
            .available_monitors()
            .into_iter()
            .map(|inner| MonitorHandle { inner })
    }

    /// Returns the primary monitor of the system.
    ///
    /// Returns `None` if it can't identify any monitor as a primary one.
    ///
    /// This is the same as [`EventLoopWindowTarget::primary_monitor`], and is provided for convenience.
    ///
    /// ## Platform-specific
    ///
    /// **iOS:** Can only be called on the main thread.
    /// **Wayland:** Always returns `None`.
    ///
    /// [`EventLoopWindowTarget::primary_monitor`]: crate::event_loop::EventLoopWindowTarget::primary_monitor
    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        self.window.primary_monitor()
    }
}
unsafe impl HasRawWindowHandle for Window {
    /// Returns a [`raw_window_handle::RawWindowHandle`] for the Window
    ///
    /// ## Platform-specific
    ///
    /// ### Android
    ///
    /// Only available after receiving [`Event::Resumed`] and before [`Event::Suspended`]. *If you
    /// try to get the handle outside of that period, this function will panic*!
    ///
    /// Make sure to release or destroy any resources created from this `RawWindowHandle` (ie. Vulkan
    /// or OpenGL surfaces) before returning from [`Event::Suspended`], at which point Android will
    /// release the underlying window/surface: any subsequent interaction is undefined behavior.
    ///
    /// [`Event::Resumed`]: crate::event::Event::Resumed
    /// [`Event::Suspended`]: crate::event::Event::Suspended
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window.raw_window_handle()
    }
}

unsafe impl HasRawDisplayHandle for Window {
    /// Returns a [`raw_window_handle::RawDisplayHandle`] used by the [`EventLoop`] that
    /// created a window.
    ///
    /// [`EventLoop`]: crate::event_loop::EventLoop
    fn raw_display_handle(&self) -> RawDisplayHandle {
        self.window.raw_display_handle()
    }
}
unsafe impl raw_window_handle_04::HasRawWindowHandle for Window {
    /// Returns a [`raw_window_handle_04::RawWindowHandle`] for the Window
    ///
    /// This provides backwards compatibility for downstream crates that have not yet
    /// upgraded to `raw_window_handle` version 0.5, such as Wgpu version 0.13.
    ///
    /// ## Platform-specific
    ///
    /// ### Android
    ///
    /// Only available after receiving [`Event::Resumed`] and before [`Event::Suspended`]. *If you
    /// try to get the handle outside of that period, this function will panic*!
    ///
    /// Make sure to release or destroy any resources created from this `RawWindowHandle` (ie. Vulkan
    /// or OpenGL surfaces) before returning from [`Event::Suspended`], at which point Android will
    /// release the underlying window/surface: any subsequent interaction is undefined behavior.
    ///
    /// [`Event::Resumed`]: crate::event::Event::Resumed
    /// [`Event::Suspended`]: crate::event::Event::Suspended
    fn raw_window_handle(&self) -> raw_window_handle_04::RawWindowHandle {
        use raw_window_handle_04::{
            AndroidNdkHandle, AppKitHandle, HaikuHandle, OrbitalHandle, UiKitHandle, WaylandHandle,
            WebHandle, Win32Handle, WinRtHandle, XcbHandle, XlibHandle,
        };

        // XXX: Ideally this would be encapsulated either through a
        // compatibility API from raw_window_handle_05 or else within the
        // backends but since this is only to provide short-term backwards
        // compatibility, we just handle the full mapping inline here.
        //
        // The intention is to remove this trait implementation before Winit
        // 0.28, once crates have had time to upgrade to raw_window_handle 0.5

        match (self.window.raw_window_handle(), self.window.raw_display_handle()) {
            (RawWindowHandle::UiKit(window_handle), _) => {
                let mut handle = UiKitHandle::empty();
                handle.ui_view = window_handle.ui_view;
                handle.ui_window = window_handle.ui_window;
                handle.ui_view_controller = window_handle.ui_view_controller;
                raw_window_handle_04::RawWindowHandle::UiKit(handle)
            },
            (RawWindowHandle::AppKit(window_handle), _) => {
                let mut handle = AppKitHandle::empty();
                handle.ns_window = window_handle.ns_window;
                handle.ns_view = window_handle.ns_view;
                raw_window_handle_04::RawWindowHandle::AppKit(handle)
            },
            (RawWindowHandle::Orbital(window_handle), _) => {
                let mut handle = OrbitalHandle::empty();
                handle.window = window_handle.window;
                raw_window_handle_04::RawWindowHandle::Orbital(handle)
            },
            (RawWindowHandle::Xlib(window_handle), RawDisplayHandle::Xlib(display_handle)) => {
                let mut handle = XlibHandle::empty();
                handle.display = display_handle.display;
                handle.window = window_handle.window;
                handle.visual_id = window_handle.visual_id;
                raw_window_handle_04::RawWindowHandle::Xlib(handle)
            },
            (RawWindowHandle::Xcb(window_handle), RawDisplayHandle::Xcb(display_handle)) => {
                let mut handle = XcbHandle::empty();
                handle.connection = display_handle.connection;
                handle.window = window_handle.window;
                handle.visual_id = window_handle.visual_id;
                raw_window_handle_04::RawWindowHandle::Xcb(handle)
            },
            (RawWindowHandle::Wayland(window_handle), RawDisplayHandle::Wayland(display_handle)) => {
                let mut handle = WaylandHandle::empty();
                handle.display = display_handle.display;
                handle.surface = window_handle.surface;
                raw_window_handle_04::RawWindowHandle::Wayland(handle)
            },
            (RawWindowHandle::Win32(window_handle), _) => {
                let mut handle = Win32Handle::empty();
                handle.hwnd = window_handle.hwnd;
                handle.hinstance = window_handle.hinstance;
                raw_window_handle_04::RawWindowHandle::Win32(handle)
            },
            (RawWindowHandle::WinRt(window_handle), _) => {
                let mut handle = WinRtHandle::empty();
                handle.core_window = window_handle.core_window;
                raw_window_handle_04::RawWindowHandle::WinRt(handle)
            },
            (RawWindowHandle::Web(window_handle), _) => {
                let mut handle = WebHandle::empty();
                handle.id = window_handle.id;
                raw_window_handle_04::RawWindowHandle::Web(handle)
            },
            (RawWindowHandle::AndroidNdk(window_handle), _) => {
                let mut handle = AndroidNdkHandle::empty();
                handle.a_native_window = window_handle.a_native_window;
                raw_window_handle_04::RawWindowHandle::AndroidNdk(handle)
            },
            (RawWindowHandle::Haiku(window_handle), _) => {
                let mut handle = HaikuHandle::empty();
                handle.b_window = window_handle.b_window;
                handle.b_direct_window = window_handle.b_direct_window;
                raw_window_handle_04::RawWindowHandle::Haiku(handle)
            },
            _ => panic!("No HasRawWindowHandle version 0.4 backwards compatibility for new Winit window type"),
        }
    }
}

/// The behavior of cursor grabbing.
///
/// Use this enum with [`Window::set_cursor_grab`] to grab the cursor.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CursorGrabMode {
    /// No grabbing of the cursor is performed.
    None,

    /// The cursor is confined to the window area.
    ///
    /// There's no guarantee that the cursor will be hidden. You should hide it by yourself if you
    /// want to do so.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Not implemented. Always returns [`ExternalError::NotSupported`] for now.
    /// - **iOS / Android / Web:** Always returns an [`ExternalError::NotSupported`].
    Confined,

    /// The cursor is locked inside the window area to the certain position.
    ///
    /// There's no guarantee that the cursor will be hidden. You should hide it by yourself if you
    /// want to do so.
    ///
    /// ## Platform-specific
    ///
    /// - **X11 / Windows:** Not implemented. Always returns [`ExternalError::NotSupported`] for now.
    /// - **iOS / Android:** Always returns an [`ExternalError::NotSupported`].
    Locked,
}

/// Describes the appearance of the mouse cursor.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CursorIcon {
    /// The platform-dependent default cursor.
    Default,
    /// A simple crosshair.
    Crosshair,
    /// A hand (often used to indicate links in web browsers).
    Hand,
    /// Self explanatory.
    Arrow,
    /// Indicates something is to be moved.
    Move,
    /// Indicates text that may be selected or edited.
    Text,
    /// Program busy indicator.
    Wait,
    /// Help indicator (often rendered as a "?")
    Help,
    /// Progress indicator. Shows that processing is being done. But in contrast
    /// with "Wait" the user may still interact with the program. Often rendered
    /// as a spinning beach ball, or an arrow with a watch or hourglass.
    Progress,

    /// Cursor showing that something cannot be done.
    NotAllowed,
    ContextMenu,
    Cell,
    VerticalText,
    Alias,
    Copy,
    NoDrop,
    /// Indicates something can be grabbed.
    Grab,
    /// Indicates something is grabbed.
    Grabbing,
    AllScroll,
    ZoomIn,
    ZoomOut,

    /// Indicate that some edge is to be moved. For example, the 'SeResize' cursor
    /// is used when the movement starts from the south-east corner of the box.
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
}

impl Default for CursorIcon {
    fn default() -> Self {
        CursorIcon::Default
    }
}

/// Fullscreen modes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fullscreen {
    Exclusive(VideoMode),

    /// Providing `None` to `Borderless` will fullscreen on the current monitor.
    Borderless(Option<MonitorHandle>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

/// ## Platform-specific
///
/// - **X11:** Sets the WM's `XUrgencyHint`. No distinction between [`Critical`] and [`Informational`].
///
/// [`Critical`]: Self::Critical
/// [`Informational`]: Self::Informational
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAttentionType {
    /// ## Platform-specific
    ///
    /// - **macOS:** Bounces the dock icon until the application is in focus.
    /// - **Windows:** Flashes both the window and the taskbar button until the application is in focus.
    Critical,
    /// ## Platform-specific
    ///
    /// - **macOS:** Bounces the dock icon once.
    /// - **Windows:** Flashes the taskbar button until the application is in focus.
    Informational,
}

impl Default for UserAttentionType {
    fn default() -> Self {
        UserAttentionType::Informational
    }
}
