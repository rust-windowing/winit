//! The `Window` struct and associated types.
use std::{fmt, error};

use platform_impl;
use event_loop::EventLoopWindowTarget;
use monitor::{AvailableMonitorsIter, MonitorHandle};
use dpi::{LogicalPosition, LogicalSize};

pub use icon::*;

/// Represents a window.
///
/// # Example
///
/// ```no_run
/// use winit::window::Window;
/// use winit::event::{Event, WindowEvent};
/// use winit::event_loop::{EventLoop, ControlFlow};
///
/// let mut event_loop = EventLoop::new();
/// let window = Window::new(&event_loop).unwrap();
///
/// event_loop.run(move |event, _, control_flow| {
///     match event {
///         Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
///             *control_flow = ControlFlow::Exit
///         },
///         _ => *control_flow = ControlFlow::Wait,
///     }
/// });
/// ```
pub struct Window {
    pub(crate) window: platform_impl::Window,
}

impl fmt::Debug for Window {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        fmtr.pad("Window { .. }")
    }
}

/// Identifier of a window. Unique for each window.
///
/// Can be obtained with `window.id()`.
///
/// Whenever you receive an event specific to a window, this event contains a `WindowId` which you
/// can then compare to the ids of your windows.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub(crate) platform_impl::WindowId);

impl WindowId {
    /// Returns a dummy `WindowId`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `WindowId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub unsafe fn dummy() -> Self {
        WindowId(platform_impl::WindowId::dummy())
    }
}

/// Object that allows you to build windows.
#[derive(Clone)]
pub struct WindowBuilder {
    /// The attributes to use to create the window.
    pub window: WindowAttributes,

    // Platform-specific configuration. Private.
    pub(crate) platform_specific: platform_impl::PlatformSpecificWindowBuilderAttributes,
}

impl fmt::Debug for WindowBuilder {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        fmtr.debug_struct("WindowBuilder")
            .field("window", &self.window)
            .finish()
    }
}

/// Attributes to use when creating a window.
#[derive(Debug, Clone)]
pub struct WindowAttributes {
    /// The dimensions of the window. If this is `None`, some platform-specific dimensions will be
    /// used.
    ///
    /// The default is `None`.
    pub dimensions: Option<LogicalSize>,

    /// The minimum dimensions a window can be, If this is `None`, the window will have no minimum dimensions (aside from reserved).
    ///
    /// The default is `None`.
    pub min_dimensions: Option<LogicalSize>,

    /// The maximum dimensions a window can be, If this is `None`, the maximum will have no maximum or will be set to the primary monitor's dimensions by the platform.
    ///
    /// The default is `None`.
    pub max_dimensions: Option<LogicalSize>,

    /// Whether the window is resizable or not.
    ///
    /// The default is `true`.
    pub resizable: bool,

    /// Whether the window should be set as fullscreen upon creation.
    ///
    /// The default is `None`.
    pub fullscreen: Option<MonitorHandle>,

    /// The title of the window in the title bar.
    ///
    /// The default is `"winit window"`.
    pub title: String,

    /// Whether the window should be maximized upon creation.
    ///
    /// The default is `false`.
    pub maximized: bool,

    /// Whether the window should be immediately visible upon creation.
    ///
    /// The default is `true`.
    pub visible: bool,

    /// Whether the the window should be transparent. If this is true, writing colors
    /// with alpha values different than `1.0` will produce a transparent window.
    ///
    /// The default is `false`.
    pub transparent: bool,

    /// Whether the window should have borders and bars.
    ///
    /// The default is `true`.
    pub decorations: bool,

    /// Whether the window should always be on top of other windows.
    ///
    /// The default is `false`.
    pub always_on_top: bool,

    /// The window icon.
    ///
    /// The default is `None`.
    pub window_icon: Option<Icon>,

    /// [iOS only] Enable multitouch,
    /// see [multipleTouchEnabled](https://developer.apple.com/documentation/uikit/uiview/1622519-multipletouchenabled)
    pub multitouch: bool,
}

impl Default for WindowAttributes {
    #[inline]
    fn default() -> WindowAttributes {
        WindowAttributes {
            dimensions: None,
            min_dimensions: None,
            max_dimensions: None,
            resizable: true,
            title: "winit window".to_owned(),
            maximized: false,
            fullscreen: None,
            visible: true,
            transparent: false,
            decorations: true,
            always_on_top: false,
            window_icon: None,
            multitouch: false,
        }
    }
}
impl WindowBuilder {
    /// Initializes a new `WindowBuilder` with default values.
    #[inline]
    pub fn new() -> WindowBuilder {
        WindowBuilder {
            window: Default::default(),
            platform_specific: Default::default(),
        }
    }

    /// Requests the window to be of specific dimensions.
    #[inline]
    pub fn with_dimensions(mut self, size: LogicalSize) -> WindowBuilder {
        self.window.dimensions = Some(size);
        self
    }

    /// Sets a minimum dimension size for the window
    #[inline]
    pub fn with_min_dimensions(mut self, min_size: LogicalSize) -> WindowBuilder {
        self.window.min_dimensions = Some(min_size);
        self
    }

    /// Sets a maximum dimension size for the window
    #[inline]
    pub fn with_max_dimensions(mut self, max_size: LogicalSize) -> WindowBuilder {
        self.window.max_dimensions = Some(max_size);
        self
    }

    /// Sets whether the window is resizable or not
    ///
    /// Note that making the window unresizable doesn't exempt you from handling `Resized`, as that event can still be
    /// triggered by DPI scaling, entering fullscreen mode, etc.
    ///
    /// ## Platform-specific
    ///
    /// This only has an effect on desktop platforms.
    ///
    /// Due to a bug in XFCE, this has no effect on Xfwm.
    #[inline]
    pub fn with_resizable(mut self, resizable: bool) -> WindowBuilder {
        self.window.resizable = resizable;
        self
    }

    /// Requests a specific title for the window.
    #[inline]
    pub fn with_title<T: Into<String>>(mut self, title: T) -> WindowBuilder {
        self.window.title = title.into();
        self
    }

    /// Sets the window fullscreen state. None means a normal window, Some(MonitorHandle)
    /// means a fullscreen window on that specific monitor
    #[inline]
    pub fn with_fullscreen(mut self, monitor: Option<MonitorHandle>) -> WindowBuilder {
        self.window.fullscreen = monitor;
        self
    }

    /// Requests maximized mode.
    #[inline]
    pub fn with_maximized(mut self, maximized: bool) -> WindowBuilder {
        self.window.maximized = maximized;
        self
    }

    /// Sets whether the window will be initially hidden or visible.
    #[inline]
    pub fn with_visibility(mut self, visible: bool) -> WindowBuilder {
        self.window.visible = visible;
        self
    }

    /// Sets whether the background of the window should be transparent.
    #[inline]
    pub fn with_transparency(mut self, transparent: bool) -> WindowBuilder {
        self.window.transparent = transparent;
        self
    }

    /// Sets whether the window should have a border, a title bar, etc.
    #[inline]
    pub fn with_decorations(mut self, decorations: bool) -> WindowBuilder {
        self.window.decorations = decorations;
        self
    }

    /// Sets whether or not the window will always be on top of other windows.
    #[inline]
    pub fn with_always_on_top(mut self, always_on_top: bool) -> WindowBuilder {
        self.window.always_on_top = always_on_top;
        self
    }

    /// Sets the window icon. On Windows and X11, this is typically the small icon in the top-left
    /// corner of the titlebar.
    ///
    /// ## Platform-specific
    ///
    /// This only has an effect on Windows and X11.
    ///
    /// On Windows, this sets `ICON_SMALL`. The base size for a window icon is 16x16, but it's
    /// recommended to account for screen scaling and pick a multiple of that, i.e. 32x32.
    ///
    /// X11 has no universal guidelines for icon sizes, so you're at the whims of the WM. That
    /// said, it's usually in the same ballpark as on Windows.
    #[inline]
    pub fn with_window_icon(mut self, window_icon: Option<Icon>) -> WindowBuilder {
        self.window.window_icon = window_icon;
        self
    }

    /// Enables multitouch.
    #[inline]
    pub fn with_multitouch(mut self) -> WindowBuilder {
        self.window.multitouch = true;
        self
    }

    /// Builds the window.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    /// out of memory, etc.
    #[inline]
    pub fn build<T: 'static>(mut self, window_target: &EventLoopWindowTarget<T>) -> Result<Window, CreationError> {
        self.window.dimensions = Some(self.window.dimensions.unwrap_or_else(|| {
            if let Some(ref monitor) = self.window.fullscreen {
                // resizing the window to the dimensions of the monitor when fullscreen
                LogicalSize::from_physical(monitor.get_dimensions(), 1.0)
            } else {
                // default dimensions
                (1024, 768).into()
            }
        }));

        // building
        platform_impl::Window::new(
            &window_target.p,
            self.window,
            self.platform_specific,
        ).map(|window| Window { window })
    }
}

impl Window {
    /// Creates a new Window for platforms where this is appropriate.
    ///
    /// This function is equivalent to `WindowBuilder::new().build(event_loop)`.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    #[inline]
    pub fn new<T: 'static>(event_loop: &EventLoopWindowTarget<T>) -> Result<Window, CreationError> {
        let builder = WindowBuilder::new();
        builder.build(event_loop)
    }

    /// Modifies the title of the window.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_title(&self, title: &str) {
        self.window.set_title(title)
    }

    /// Shows the window if it was hidden.
    ///
    /// ## Platform-specific
    ///
    /// - Has no effect on Android
    ///
    #[inline]
    pub fn show(&self) {
        self.window.show()
    }

    /// Hides the window if it was visible.
    ///
    /// ## Platform-specific
    ///
    /// - Has no effect on Android
    ///
    #[inline]
    pub fn hide(&self) {
        self.window.hide()
    }

    /// Emits a `WindowEvent::RedrawRequested` event in the associated event loop after all OS
    /// events have been processed by the event loop.
    ///
    /// This is the **strongly encouraged** method of redrawing windows, as it can integrates with
    /// OS-requested redraws (e.g. when a window gets resized).
    ///
    /// This function can cause `RedrawRequested` events to be emitted after `Event::EventsCleared`
    /// but before `Event::NewEvents` if called in the following circumstances:
    /// * While processing `EventsCleared`.
    /// * While processing a `RedrawRequested` event that was sent during `EventsCleared` or any
    ///   directly subsequent `RedrawRequested` event.
    pub fn request_redraw(&self) {
        self.window.request_redraw()
    }

    /// Returns the position of the top-left hand corner of the window relative to the
    ///  top-left hand corner of the desktop.
    ///
    /// Note that the top-left hand corner of the desktop is not necessarily the same as
    ///  the screen. If the user uses a desktop with multiple monitors, the top-left hand corner
    ///  of the desktop is the top-left hand corner of the monitor at the top-left of the desktop.
    ///
    /// The coordinates can be negative if the top-left hand corner of the window is outside
    ///  of the visible screen region.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_position(&self) -> Option<LogicalPosition> {
        self.window.get_position()
    }

    /// Returns the position of the top-left hand corner of the window's client area relative to the
    /// top-left hand corner of the desktop.
    ///
    /// The same conditions that apply to `get_position` apply to this method.
    #[inline]
    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        self.window.get_inner_position()
    }

    /// Modifies the position of the window.
    ///
    /// See `get_position` for more information about the coordinates.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_position(&self, position: LogicalPosition) {
        self.window.set_position(position)
    }

    /// Returns the logical size of the window's client area.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    ///
    /// Converting the returned `LogicalSize` to `PhysicalSize` produces the size your framebuffer should be.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        self.window.get_inner_size()
    }

    /// Returns the logical size of the entire window.
    ///
    /// These dimensions include the title bar and borders. If you don't want that (and you usually don't),
    /// use `get_inner_size` instead.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        self.window.get_outer_size()
    }

    /// Modifies the inner size of the window.
    ///
    /// See `get_inner_size` for more information about the values.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        self.window.set_inner_size(size)
    }

    /// Sets a minimum dimension size for the window.
    #[inline]
    pub fn set_min_dimensions(&self, dimensions: Option<LogicalSize>) {
        self.window.set_min_dimensions(dimensions)
    }

    /// Sets a maximum dimension size for the window.
    #[inline]
    pub fn set_max_dimensions(&self, dimensions: Option<LogicalSize>) {
        self.window.set_max_dimensions(dimensions)
    }

    /// Sets whether the window is resizable or not.
    ///
    /// Note that making the window unresizable doesn't exempt you from handling `Resized`, as that event can still be
    /// triggered by DPI scaling, entering fullscreen mode, etc.
    ///
    /// ## Platform-specific
    ///
    /// This only has an effect on desktop platforms.
    ///
    /// Due to a bug in XFCE, this has no effect on Xfwm.
    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        self.window.set_resizable(resizable)
    }

    /// Returns the DPI factor that can be used to map logical pixels to physical pixels, and vice versa.
    ///
    /// See the [`dpi`](dpi/index.html) module for more information.
    ///
    /// Note that this value can change depending on user action (for example if the window is
    /// moved to another screen); as such, tracking `WindowEvent::HiDpiFactorChanged` events is
    /// the most robust way to track the DPI you need to use to draw.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** This respects Xft.dpi, and can be overridden using the `WINIT_HIDPI_FACTOR` environment variable.
    /// - **Android:** Always returns 1.0.
    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        self.window.get_hidpi_factor()
    }

    /// Modifies the mouse cursor of the window.
    /// Has no effect on Android.
    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        self.window.set_cursor(cursor);
    }

    /// Changes the position of the cursor in window coordinates.
    #[inline]
    pub fn set_cursor_position(&self, position: LogicalPosition) -> Result<(), String> {
        self.window.set_cursor_position(position)
    }

    /// Grabs the cursor, preventing it from leaving the window.
    ///
    /// ## Platform-specific
    ///
    /// On macOS, this presently merely locks the cursor in a fixed location, which looks visually awkward.
    ///
    /// This has no effect on Android or iOS.
    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        self.window.grab_cursor(grab)
    }

    /// Hides the cursor, making it invisible but still usable.
    ///
    /// ## Platform-specific
    ///
    /// On Windows and X11, the cursor is only hidden within the confines of the window.
    ///
    /// On macOS, the cursor is hidden as long as the window has input focus, even if the cursor is outside of the
    /// window.
    ///
    /// This has no effect on Android or iOS.
    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        self.window.hide_cursor(hide)
    }

    /// Sets the window to maximized or back
    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        self.window.set_maximized(maximized)
    }

    /// Sets the window to fullscreen or back
    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<MonitorHandle>) {
        self.window.set_fullscreen(monitor)
    }

    /// Turn window decorations on or off.
    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        self.window.set_decorations(decorations)
    }

    /// Change whether or not the window will always be on top of other windows.
    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        self.window.set_always_on_top(always_on_top)
    }

    /// Sets the window icon. On Windows and X11, this is typically the small icon in the top-left
    /// corner of the titlebar.
    ///
    /// For more usage notes, see `WindowBuilder::with_window_icon`.
    ///
    /// ## Platform-specific
    ///
    /// This only has an effect on Windows and X11.
    #[inline]
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        self.window.set_window_icon(window_icon)
    }

    /// Sets location of IME candidate box in client area coordinates relative to the top left.
    #[inline]
    pub fn set_ime_spot(&self, position: LogicalPosition) {
        self.window.set_ime_spot(position)
    }

    /// Returns the monitor on which the window currently resides
    #[inline]
    pub fn get_current_monitor(&self) -> MonitorHandle {
        self.window.get_current_monitor()
    }

    /// Returns the list of all the monitors available on the system.
    ///
    /// This is the same as `EventLoop::get_available_monitors`, and is provided for convenience.
    #[inline]
    pub fn get_available_monitors(&self) -> AvailableMonitorsIter {
        let data = self.window.get_available_monitors();
        AvailableMonitorsIter { data: data.into_iter() }
    }

    /// Returns the primary monitor of the system.
    ///
    /// This is the same as `EventLoop::get_primary_monitor`, and is provided for convenience.
    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle { inner: self.window.get_primary_monitor() }
    }

    /// Returns an identifier unique to the window.
    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.window.id())
    }
}

/// Error that can happen while creating a window or a headless renderer.
#[derive(Debug, Clone)]
pub enum CreationError {
    OsError(String),
    /// TODO: remove this error
    NotSupported,
}

impl CreationError {
    fn to_string(&self) -> &str {
        match *self {
            CreationError::OsError(ref text) => &text,
            CreationError::NotSupported => "Some of the requested attributes are not supported",
        }
    }
}

impl fmt::Display for CreationError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        formatter.write_str(self.to_string())
    }
}

impl error::Error for CreationError {
    fn description(&self) -> &str {
        self.to_string()
    }
}

/// Describes the appearance of the mouse cursor.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MouseCursor {
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
    Grab,
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

impl Default for MouseCursor {
    fn default() -> Self {
        MouseCursor::Default
    }
}
