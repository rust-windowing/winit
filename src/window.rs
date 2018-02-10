use std::collections::vec_deque::IntoIter as VecDequeIter;

use CreationError;
use CursorState;
use EventsLoop;
use MouseCursor;
use Window;
use WindowBuilder;
use WindowId;

use libc;
use platform;

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
    ///
    /// Width and height are in pixels.
    #[inline]
    pub fn with_dimensions(mut self, width: u32, height: u32) -> WindowBuilder {
        self.window.dimensions = Some((width, height));
        self
    }

    /// Sets a minimum dimension size for the window
    ///
    /// Width and height are in pixels.
    #[inline]
    pub fn with_min_dimensions(mut self, width: u32, height: u32) -> WindowBuilder {
        self.window.min_dimensions = Some((width, height));
        self
    }

    /// Sets a maximum dimension size for the window
    ///
    /// Width and height are in pixels.
    #[inline]
    pub fn with_max_dimensions(mut self, width: u32, height: u32) -> WindowBuilder {
        self.window.max_dimensions = Some((width, height));
        self
    }

    /// Requests a specific title for the window.
    #[inline]
    pub fn with_title<T: Into<String>>(mut self, title: T) -> WindowBuilder {
        self.window.title = title.into();
        self
    }

    /// Sets the window fullscreen state. None means a normal window, Some(MonitorId)
    /// means a fullscreen window on that specific monitor
    #[inline]
    pub fn with_fullscreen(mut self, monitor: Option<MonitorId>) -> WindowBuilder {
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
    pub fn build(mut self, events_loop: &EventsLoop) -> Result<Window, CreationError> {
        // resizing the window to the dimensions of the monitor when fullscreen
        if self.window.dimensions.is_none() {
            if let Some(ref monitor) = self.window.fullscreen {
                self.window.dimensions = Some(monitor.get_dimensions());
            }
        }

        // default dimensions
        if self.window.dimensions.is_none() {
            self.window.dimensions = Some((1024, 768));
        }

        // building
        let w = try!(platform::Window::new(&events_loop.events_loop, &self.window, &self.platform_specific));

        Ok(Window { window: w })
    }
}

impl Window {
    /// Creates a new Window for platforms where this is appropriate.
    ///
    /// This function is equivalent to `WindowBuilder::new().build(events_loop)`.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    #[inline]
    pub fn new(events_loop: &EventsLoop) -> Result<Window, CreationError> {
        let builder = WindowBuilder::new();
        builder.build(events_loop)
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
    pub fn get_position(&self) -> Option<(i32, i32)> {
        self.window.get_position()
    }

    /// Modifies the position of the window.
    ///
    /// See `get_position` for more information about the coordinates.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_position(&self, x: i32, y: i32) {
        self.window.set_position(x, y)
    }

    /// Returns the size in pixels of the client area of the window.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    /// These are the dimensions that need to be supplied to `glViewport`.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        self.window.get_inner_size()
    }

    /// Returns the size in points of the client area of the window.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    /// To get the dimensions of the frame buffer when calling `glViewport`, multiply with hidpi factor.
    ///
    /// Returns `None` if the window no longer exists.
    ///
    /// DEPRECATED
    #[inline]
    #[deprecated]
    pub fn get_inner_size_points(&self) -> Option<(u32, u32)> {
        self.window.get_inner_size().map(|(x, y)| {
            let hidpi = self.hidpi_factor();
            ((x as f32 / hidpi) as u32, (y as f32 / hidpi) as u32)
        })
    }

    /// Returns the size in pixels of the client area of the window.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    /// These are the dimensions of the frame buffer, and the dimensions that you should use
    ///  when you call `glViewport`.
    ///
    /// Returns `None` if the window no longer exists.
    ///
    /// DEPRECATED
    #[inline]
    #[deprecated]
    pub fn get_inner_size_pixels(&self) -> Option<(u32, u32)> {
        self.window.get_inner_size()
    }

    /// Returns the size in pixels of the window.
    ///
    /// These dimensions include title bar and borders. If you don't want these, you should use
    ///  use `get_inner_size` instead.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.window.get_outer_size()
    }

    /// Modifies the inner size of the window.
    ///
    /// See `get_inner_size` for more information about the values.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        self.window.set_inner_size(x, y)
    }

    /// Sets a minimum dimension size for the window.
    ///
    /// Width and height are in pixels.
    #[inline]
    pub fn set_min_dimensions(&self, dimensions: Option<(u32, u32)>) {
        self.window.set_min_dimensions(dimensions)
    }

    /// Sets a maximum dimension size for the window.
    ///
    /// Width and height are in pixels.
    #[inline]
    pub fn set_max_dimensions(&self, dimensions: Option<(u32, u32)>) {
        self.window.set_max_dimensions(dimensions)
    }

    /// DEPRECATED. Gets the native platform specific display for this window.
    /// This is typically only required when integrating with
    /// other libraries that need this information.
    #[deprecated]
    #[inline]
    pub unsafe fn platform_display(&self) -> *mut libc::c_void {
        self.window.platform_display()
    }

    /// DEPRECATED. Gets the native platform specific window handle. This is
    /// typically only required when integrating with other libraries
    /// that need this information.
    #[deprecated]
    #[inline]
    pub unsafe fn platform_window(&self) -> *mut libc::c_void {
        self.window.platform_window()
    }

    /// Modifies the mouse cursor of the window.
    /// Has no effect on Android.
    pub fn set_cursor(&self, cursor: MouseCursor) {
        self.window.set_cursor(cursor);
    }

    /// Returns the ratio between the backing framebuffer resolution and the
    /// window size in screen pixels. This is typically one for a normal display
    /// and two for a retina display.
    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        self.window.hidpi_factor()
    }

    /// Changes the position of the cursor in window coordinates.
    #[inline]
    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        self.window.set_cursor_position(x, y)
    }

    /// Sets how winit handles the cursor. See the documentation of `CursorState` for details.
    ///
    /// Has no effect on Android.
    #[inline]
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        self.window.set_cursor_state(state)
    }

    /// Sets the window to maximized or back
    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        self.window.set_maximized(maximized)
    }

    /// Sets the window to fullscreen or back
    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<MonitorId>) {
        self.window.set_fullscreen(monitor)
    }

    /// Turn window decorations on or off.
    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        self.window.set_decorations(decorations)
    }

    /// Returns the current monitor the window is on or the primary monitor is nothing
    /// matches
    pub fn get_current_monitor(&self) -> MonitorId {
        self.window.get_current_monitor()
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.window.id())
    }
}

/// An iterator for the list of available monitors.
// Implementation note: we retrieve the list once, then serve each element by one by one.
// This may change in the future.
pub struct AvailableMonitorsIter {
    pub(crate) data: VecDequeIter<platform::MonitorId>,
}

impl Iterator for AvailableMonitorsIter {
    type Item = MonitorId;

    #[inline]
    fn next(&mut self) -> Option<MonitorId> {
        self.data.next().map(|id| MonitorId { inner: id })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.data.size_hint()
    }
}

/// Identifier for a monitor.
#[derive(Clone)]
pub struct MonitorId {
    pub(crate) inner: platform::MonitorId
}

impl MonitorId {
    /// Returns a human-readable name of the monitor.
    ///
    /// Returns `None` if the monitor doesn't exist anymore.
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        self.inner.get_name()
    }

    /// Returns the number of pixels currently displayed on the monitor.
    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        self.inner.get_dimensions()
    }

    /// Returns the top-left corner position of the monitor relative to the larger full
    /// screen area.
    #[inline]
    pub fn get_position(&self) -> (i32, i32) {
        self.inner.get_position()
    }

    /// Returns the ratio between the monitor's physical pixels and logical pixels.
    #[inline]
    pub fn get_hidpi_factor(&self) -> f32 {
        self.inner.get_hidpi_factor()
    }
}
