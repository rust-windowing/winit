#![feature(unsafe_destructor)]
#![feature(globs)]
#![unstable]

extern crate libc;

pub use events::*;

#[cfg(windows)]
use winimpl = win32;
#[cfg(unix)]
use winimpl = x11;

#[cfg(windows)]
mod win32;
#[cfg(unix)]
mod x11;

#[allow(dead_code)]
//mod egl;

mod events;

/// Identifier for a monitor.
pub struct MonitorID(winimpl::MonitorID);

/// Object that allows you to build windows.
pub struct WindowBuilder {
    dimensions: (uint, uint),
    title: String,
    monitor: Option<winimpl::MonitorID>,
    gl_version: Option<(uint, uint)>,
}

impl WindowBuilder {
    /// Initializes a new `WindowBuilder` with default values.
    pub fn new() -> WindowBuilder {
        WindowBuilder {
            dimensions: (1024, 768),
            title: String::new(),
            monitor: None,
            gl_version: None,
        }
    }

    pub fn with_dimensions(mut self, width: uint, height: uint) -> WindowBuilder {
        self.dimensions = (width, height);
        self
    }

    pub fn with_title(mut self, title: String) -> WindowBuilder {
        self.title = title;
        self
    }

    pub fn with_monitor(mut self, monitor: MonitorID) -> WindowBuilder {
        let MonitorID(monitor) = monitor;
        self.monitor = Some(monitor);
        self
    }

    /// Requests to use a specific OpenGL version.
    ///
    /// Version is a (major, minor) pair. For example to request OpenGL 3.3
    ///  you would pass `(3, 3)`.
    pub fn with_gl_version(mut self, version: (uint, uint)) -> WindowBuilder {
        self.gl_version = Some(version);
        self
    }

    /// Builds the window.
    /// 
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    pub fn build(self) -> Result<Window, String> {
        winimpl::Window::new(self).map(|w| Window { window: w })
    }
}

/// Represents an OpenGL context and the Window or environment around it.
///
/// # Example
///
/// ```
/// let window = Window::new().unwrap();
/// 
/// unsafe { window.make_current() };
/// 
/// loop {
///     for event in window.poll_events().move_iter() {     // note: this may change in the future
///         match event {
///             // process events here
///             _ => ()
///         }
///     }
///     
///     // draw everything here
///
///     window.swap_buffers();
///     std::io::timer::sleep(17);
/// }
/// ```
pub struct Window {
    window: winimpl::Window,
}

impl Window {
    /// Creates a new OpenGL context, and a Window for platforms where this is appropriate.
    ///
    /// This function is equivalent to `WindowBuilder::new().build()`.
    /// 
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    #[inline]
    pub fn new() -> Result<Window, String> {
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

    /// Returns the position of the top-left hand corner of the window relative to the
    ///  top-left hand corner of the desktop.
    ///
    /// Note that the top-left hand corner of the desktop is not necessarly the same as
    ///  the screen. If the user uses a desktop with multiple monitors, the top-left hand corner
    ///  of the desktop is the top-left hand corner of the monitor at the top-left of the desktop.
    ///
    /// The coordinates can be negative if the top-left hand corner of the window is outside
    ///  of the visible screen region.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_position(&self) -> Option<(int, int)> {
        self.window.get_position()
    }

    /// Modifies the position of the window.
    ///
    /// See `get_position` for more informations about the coordinates.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_position(&self, x: uint, y: uint) {
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
    pub fn get_inner_size(&self) -> Option<(uint, uint)> {
        self.window.get_inner_size()
    }

    /// Returns the size in pixels of the window.
    ///
    /// These dimensions include title bar and borders. If you don't want these, you should use
    ///  use `get_inner_size` instead.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_outer_size(&self) -> Option<(uint, uint)> {
        self.window.get_outer_size()
    }

    /// Modifies the inner size of the window.
    ///
    /// See `get_inner_size` for more informations about the values.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_inner_size(&self, x: uint, y: uint) {
        self.window.set_inner_size(x, y)
    }

    /// Returns an iterator to all the events that are currently in the window's events queue.
    /// 
    /// Contrary to `wait_events`, this function never blocks.
    #[inline]
    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator { data: self.window.poll_events() }
    }

    /// Waits for an event, then returns an iterator to all the events that are currently
    ///  in the window's events queue.
    /// 
    /// If there are no events in queue when you call the function,
    ///  this function will block until there is one.
    #[inline]
    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator { data: self.window.wait_events() }
    }

    /// Sets the context as the current context.
    #[inline]
    #[experimental]
    pub unsafe fn make_current(&self) {
        self.window.make_current()
    }

    /// Returns the address of an OpenGL function.
    ///
    /// Contrary to `wglGetProcAddress`, all available OpenGL functions return an address.
    #[inline]
    pub fn get_proc_address(&self, addr: &str) -> *const () {
        self.window.get_proc_address(addr)
    }

    /// Swaps the buffers in case of double or triple buffering.
    ///
    /// You should call this function every time you have finished rendering, or the image
    ///  may not be displayed on the screen.
    #[inline]
    pub fn swap_buffers(&self) {
        self.window.swap_buffers()
    }
}

/// An iterator for the `poll_events` function.
// Implementation note: we retreive the list once, then serve each element by one by one.
// This may change in the future.
pub struct PollEventsIterator<'a> {
    data: Vec<Event>,
}

impl<'a> Iterator<Event> for PollEventsIterator<'a> {
    fn next(&mut self) -> Option<Event> {
        self.data.remove(0)
    }
}

/// An iterator for the `wait_events` function.
// Implementation note: we retreive the list once, then serve each element by one by one.
// This may change in the future.
pub struct WaitEventsIterator<'a> {
    data: Vec<Event>,
}

impl<'a> Iterator<Event> for WaitEventsIterator<'a> {
    fn next(&mut self) -> Option<Event> {
        self.data.remove(0)
    }
}

/// An iterator for the list of available monitors.
// Implementation note: we retreive the list once, then serve each element by one by one.
// This may change in the future.
pub struct AvailableMonitorsIter {
    data: Vec<winimpl::MonitorID>,
}

impl Iterator<MonitorID> for AvailableMonitorsIter {
    fn next(&mut self) -> Option<MonitorID> {
        self.data.remove(0).map(|id| MonitorID(id))
    }
}

/// Returns the list of all available monitors.
pub fn get_available_monitors() -> AvailableMonitorsIter {
    let data = winimpl::get_available_monitors();
    AvailableMonitorsIter{ data: data }
}

/// Returns the primary monitor of the system.
pub fn get_primary_monitor() -> MonitorID {
    MonitorID(winimpl::get_primary_monitor())
}

impl MonitorID {
    /// Returns a human-readable name of the monitor.
    pub fn get_name(&self) -> Option<String> {
        let &MonitorID(ref id) = self;
        id.get_name()
    }
}
