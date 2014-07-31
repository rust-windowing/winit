#![feature(unsafe_destructor)]
#![feature(globs)]
#![unstable]

extern crate libc;

pub use events::*;
pub use hints::{Hints, ClientAPI, Profile};

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
mod hints;

/// Identifier for a monitor.
pub struct MonitorID(winimpl::MonitorID);

/// Represents an OpenGL context and the Window or environment around it.
///
/// # Example
///
/// ```
/// use std::default::Default;
/// 
/// let window = Window::new(None, "Hello world!", &Default::default(), None).unwrap();
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
    nosend: std::kinds::marker::NoSend,
}

impl Window {
    /// Creates a new OpenGL context, and a Window for platforms where this is appropriate.
    /// 
    /// # Parameters
    /// 
    /// The `dimensions` parameter tell the library what the dimensions of the client area
    ///  of the window must be. If set to `None`, the library will choose or let the O/S choose.
    ///
    /// The `title` parameter is the title that the window must have.
    ///
    /// The `hints` parameter must be a `Hint` object which contains hints about how the context
    ///  must be created. This library will *try* to follow the hints, but will still success
    ///  even if it could not conform to all of them.
    ///
    /// The `monitor` parameter is the identifier of the monitor that this window should fill.
    ///  If `None`, a windowed window will be created. If `Some(_)`, the window will be fullscreen
    ///  and will fill the given monitor. Note `MonitorID` does not necessarly represent a
    ///  *physical* monitor.
    ///
    /// # Return value
    ///
    /// Returns the `Window` object.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    #[inline]
    pub fn new(dimensions: Option<(uint, uint)>, title: &str,
        hints: &Hints, monitor: Option<MonitorID>)
        -> Result<Window, String>
    {
        // extracting the monitor ID
        let monitor = monitor.map(|id| { let MonitorID(id) = id; id });

        // creating the window
        let win = try!(winimpl::Window::new(dimensions, title, hints, monitor));

        Ok(Window{
            window: win,
            nosend: std::kinds::marker::NoSend,
        })
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

    /// Returns all the events that are currently in window's events queue.
    /// 
    /// Contrary to `wait_events`, this function never blocks.
    #[experimental = "Will probably be changed to return an iterator instead of a Vec"]
    #[inline]
    pub fn poll_events(&self) -> Vec<Event> {
        self.window.poll_events()
    }

    /// Returns all the events that are currently in window's events queue.
    /// If there are no events in queue, this function will block until there is one.
    ///
    /// This is equivalent to:
    ///
    /// ```
    /// loop {
    ///     let events = poll_events();
    ///     if events.len() >= 1 { return events }
    /// }
    /// ```
    ///
    /// ...but without the spinlock.
    #[inline]
    #[experimental = "Will probably be changed to return an iterator instead of a Vec"]
    pub fn wait_events(&self) -> Vec<Event> {
        self.window.wait_events()
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
