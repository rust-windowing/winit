use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::default::Default;

use Api;
use BuilderAttribs;
use CreationError;
use CursorState;
use Event;
use GlRequest;
use MouseCursor;
use native_monitor::NativeMonitorId;

use gl_common;
use libc;

use platform;

/// Object that allows you to build windows.
pub struct WindowBuilder<'a> {
    attribs: BuilderAttribs<'a>
}

impl<'a> WindowBuilder<'a> {
    /// Initializes a new `WindowBuilder` with default values.
    pub fn new() -> WindowBuilder<'a> {
        WindowBuilder {
            attribs: BuilderAttribs::new(),
        }
    }

    /// Requests the window to be of specific dimensions.
    ///
    /// Width and height are in pixels.
    pub fn with_dimensions(mut self, width: u32, height: u32) -> WindowBuilder<'a> {
        self.attribs.dimensions = Some((width, height));
        self
    }

    /// Requests a specific title for the window.
    pub fn with_title(mut self, title: String) -> WindowBuilder<'a> {
        self.attribs.title = title;
        self
    }

    /// Requests fullscreen mode.
    ///
    /// If you don't specify dimensions for the window, it will match the monitor's.
    pub fn with_fullscreen(mut self, monitor: MonitorID) -> WindowBuilder<'a> {
        let MonitorID(monitor) = monitor;
        self.attribs.monitor = Some(monitor);
        self
    }

    /// The created window will share all its OpenGL objects with the window in the parameter.
    ///
    /// There are some exceptions, like FBOs or VAOs. See the OpenGL documentation.
    pub fn with_shared_lists(mut self, other: &'a Window) -> WindowBuilder<'a> {
        self.attribs.sharing = Some(&other.window);
        self
    }

    /// THIS FUNCTION IS DEPRECATED
    #[deprecated = "Use with_gl instead"]
    pub fn with_gl_version(mut self, version: (u32, u32)) -> WindowBuilder<'a> {
        self.attribs.gl_version = GlRequest::Specific(::Api::OpenGl, (version.0 as u8, version.1 as u8));
        self
    }

    /// Sets how the backend should choose the OpenGL API and version.
    pub fn with_gl(mut self, request: GlRequest) -> WindowBuilder<'a> {
        self.attribs.gl_version = request;
        self
    }

    /// Sets the *debug* flag for the OpenGL context.
    ///
    /// The default value for this flag is `cfg!(debug_assertions)`, which means that it's enabled
    /// when you run `cargo build` and disabled when you run `cargo build --release`.
    pub fn with_gl_debug_flag(mut self, flag: bool) -> WindowBuilder<'a> {
        self.attribs.gl_debug = flag;
        self
    }

    /// Requests that the window has vsync enabled.
    pub fn with_vsync(mut self) -> WindowBuilder<'a> {
        self.attribs.vsync = true;
        self
    }

    /// Sets whether the window will be initially hidden or visible.
    pub fn with_visibility(mut self, visible: bool) -> WindowBuilder<'a> {
        self.attribs.visible = visible;
        self
    }

    /// Sets the multisampling level to request.
    ///
    /// # Panic
    ///
    /// Will panic if `samples` is not a power of two.
    pub fn with_multisampling(mut self, samples: u16) -> WindowBuilder<'a> {
        assert!(samples.is_power_of_two());
        self.attribs.multisampling = Some(samples);
        self
    }

    /// Sets the number of bits in the depth buffer.
    pub fn with_depth_buffer(mut self, bits: u8) -> WindowBuilder<'a> {
        self.attribs.depth_bits = Some(bits);
        self
    }

    /// Sets the number of bits in the stencil buffer.
    pub fn with_stencil_buffer(mut self, bits: u8) -> WindowBuilder<'a> {
        self.attribs.stencil_bits = Some(bits);
        self
    }

    /// Sets the number of bits in the color buffer.
    pub fn with_pixel_format(mut self, color_bits: u8, alpha_bits: u8) -> WindowBuilder<'a> {
        self.attribs.color_bits = Some(color_bits);
        self.attribs.alpha_bits = Some(alpha_bits);
        self
    }

    /// Request the backend to be stereoscopic.
    pub fn with_stereoscopy(mut self) -> WindowBuilder<'a> {
        self.attribs.stereoscopy = true;
        self
    }

    /// Sets whether sRGB should be enabled on the window. `None` means "I don't care".
    pub fn with_srgb(mut self, srgb_enabled: Option<bool>) -> WindowBuilder<'a> {
        self.attribs.srgb = srgb_enabled;
        self
    }

    /// Builds the window.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    pub fn build(mut self) -> Result<Window, CreationError> {
        // resizing the window to the dimensions of the monitor when fullscreen
        if self.attribs.dimensions.is_none() && self.attribs.monitor.is_some() {
            self.attribs.dimensions = Some(self.attribs.monitor.as_ref().unwrap().get_dimensions())
        }

        // default dimensions
        if self.attribs.dimensions.is_none() {
            self.attribs.dimensions = Some((1024, 768));
        }

        // building
        platform::Window::new(self.attribs).map(|w| Window { window: w })
    }

    /// Builds the window.
    ///
    /// The context is build in a *strict* way. That means that if the backend couldn't give
    /// you what you requested, an `Err` will be returned.
    pub fn build_strict(mut self) -> Result<Window, CreationError> {
        self.attribs.strict = true;
        self.build()
    }
}

/// Represents an OpenGL context and the Window or environment around it.
///
/// # Example
///
/// ```ignore
/// let window = Window::new().unwrap();
///
/// unsafe { window.make_current() };
///
/// loop {
///     for event in window.poll_events() {
///         match(event) {
///             // process events here
///             _ => ()
///         }
///     }
///
///     // draw everything here
///
///     window.swap_buffers();
///     std::old_io::timer::sleep(17);
/// }
/// ```
pub struct Window {
    window: platform::Window,
}

impl Default for Window {
    fn default() -> Window {
        Window::new().unwrap()
    }
}

impl Window {
    /// Creates a new OpenGL context, and a Window for platforms where this is appropriate.
    ///
    /// This function is equivalent to `WindowBuilder::new().build()`.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    #[inline]
    pub fn new() -> Result<Window, CreationError> {
        let builder = WindowBuilder::new();
        builder.build()
    }

    /// Returns true if the window has previously been closed by the user.
    #[inline]
    pub fn is_closed(&self) -> bool {
        self.window.is_closed()
    }

    /// Returns true if the window has previously been closed by the user.
    #[inline]
    #[deprecated = "Use is_closed instead"]
    pub fn should_close(&self) -> bool {
        self.is_closed()
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
    /// See `get_position` for more informations about the coordinates.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_position(&self, x: i32, y: i32) {
        self.window.set_position(x, y)
    }

    /// Returns the size in pixels of the client area of the window.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    /// These are the dimensions of the frame buffer, and the dimensions that you should use
    ///  when you call `glViewport`.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
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
    /// See `get_inner_size` for more informations about the values.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        self.window.set_inner_size(x, y)
    }

    /// Returns an iterator that poll for the next event in the window's events queue.
    /// Returns `None` if there is no event in the queue.
    ///
    /// Contrary to `wait_events`, this function never blocks.
    #[inline]
    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator(self.window.poll_events())
    }

    /// Returns an iterator that returns events one by one, blocking if necessary until one is
    /// available.
    ///
    /// The iterator never returns `None`.
    #[inline]
    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator(self.window.wait_events())
    }

    /// Sets the context as the current context.
    #[inline]
    pub unsafe fn make_current(&self) {
        self.window.make_current()
    }

    /// Returns true if this context is the current one in this thread.
    #[inline]
    pub fn is_current(&self) -> bool {
        self.window.is_current()
    }

    /// Returns the address of an OpenGL function.
    ///
    /// Contrary to `wglGetProcAddress`, all available OpenGL functions return an address.
    #[inline]
    pub fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        self.window.get_proc_address(addr) as *const libc::c_void
    }

    /// Swaps the buffers in case of double or triple buffering.
    ///
    /// You should call this function every time you have finished rendering, or the image
    ///  may not be displayed on the screen.
    ///
    /// **Warning**: if you enabled vsync, this function will block until the next time the screen
    /// is refreshed. However drivers can choose to override your vsync settings, which means that
    /// you can't know in advance whether `swap_buffers` will block or not.
    #[inline]
    pub fn swap_buffers(&self) {
        self.window.swap_buffers()
    }

    /// Gets the native platform specific display for this window.
    /// This is typically only required when integrating with
    /// other libraries that need this information.
    #[inline]
    pub unsafe fn platform_display(&self) -> *mut libc::c_void {
        self.window.platform_display()
    }

    /// Gets the native platform specific window handle. This is
    /// typically only required when integrating with other libraries
    /// that need this information.
    #[inline]
    pub unsafe fn platform_window(&self) -> *mut libc::c_void {
        self.window.platform_window()
    }

    /// Returns the API that is currently provided by this window.
    ///
    /// - On Windows and OS/X, this always returns `OpenGl`.
    /// - On Android, this always returns `OpenGlEs`.
    /// - On Linux, it must be checked at runtime.
    pub fn get_api(&self) -> Api {
        self.window.get_api()
    }

    /// Create a window proxy for this window, that can be freely
    /// passed to different threads.
    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy {
            proxy: self.window.create_window_proxy()
        }
    }

    /// Sets a resize callback that is called by Mac (and potentially other
    /// operating systems) during resize operations. This can be used to repaint
    /// during window resizing.
    pub fn set_window_resize_callback(&mut self, callback: Option<fn(u32, u32)>) {
        self.window.set_window_resize_callback(callback);
    }

    /// Modifies the mouse cursor of the window.
    /// Has no effect on Android.
    pub fn set_cursor(&self, cursor: MouseCursor) {
        self.window.set_cursor(cursor);
    }

    /// Returns the ratio between the backing framebuffer resolution and the
    /// window size in screen pixels. This is typically one for a normal display
    /// and two for a retina display.
    pub fn hidpi_factor(&self) -> f32 {
        self.window.hidpi_factor()
    }

    /// Changes the position of the cursor in window coordinates.
    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        self.window.set_cursor_position(x, y)
    }

    /// Sets how glutin handles the cursor. See the documentation of `CursorState` for details.
    ///
    /// Has no effect on Android.
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        self.window.set_cursor_state(state)
    }
}

impl gl_common::GlFunctionsSource for Window {
    fn get_proc_addr(&self, addr: &str) -> *const libc::c_void {
        self.get_proc_address(addr)
    }
}

/// Represents a thread safe subset of operations that can be called
/// on a window. This structure can be safely cloned and sent between
/// threads.
#[derive(Clone)]
pub struct WindowProxy {
    proxy: platform::WindowProxy,
}

impl WindowProxy {

    /// Triggers a blocked event loop to wake up. This is
    /// typically called when another thread wants to wake
    /// up the blocked rendering thread to cause a refresh.
    #[inline]
    pub fn wakeup_event_loop(&self) {
        self.proxy.wakeup_event_loop();
    }
}
/// An iterator for the `poll_events` function.
pub struct PollEventsIterator<'a>(platform::PollEventsIterator<'a>);

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// An iterator for the `wait_events` function.
pub struct WaitEventsIterator<'a>(platform::WaitEventsIterator<'a>);

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// An iterator for the list of available monitors.
// Implementation note: we retreive the list once, then serve each element by one by one.
// This may change in the future.
pub struct AvailableMonitorsIter {
    data: VecDequeIter<platform::MonitorID>,
}

impl Iterator for AvailableMonitorsIter {
    type Item = MonitorID;

    fn next(&mut self) -> Option<MonitorID> {
        self.data.next().map(|id| MonitorID(id))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.data.size_hint()
    }
}

/// Returns the list of all available monitors.
pub fn get_available_monitors() -> AvailableMonitorsIter {
    let data = platform::get_available_monitors();
    AvailableMonitorsIter{ data: data.into_iter() }
}

/// Returns the primary monitor of the system.
pub fn get_primary_monitor() -> MonitorID {
    MonitorID(platform::get_primary_monitor())
}

/// Identifier for a monitor.
pub struct MonitorID(platform::MonitorID);

impl MonitorID {
    /// Returns a human-readable name of the monitor.
    pub fn get_name(&self) -> Option<String> {
        let &MonitorID(ref id) = self;
        id.get_name()
    }

    /// Returns the native platform identifier for this monitor.
    pub fn get_native_identifier(&self) -> NativeMonitorId {
        let &MonitorID(ref id) = self;
        id.get_native_identifier()
    }

    /// Returns the number of pixels currently displayed on the monitor.
    pub fn get_dimensions(&self) -> (u32, u32) {
        let &MonitorID(ref id) = self;
        id.get_dimensions()
    }
}
