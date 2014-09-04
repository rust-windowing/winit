#![feature(unsafe_destructor)]
#![feature(globs)]
#![feature(phase)]
#![unstable]

//! The purpose of this library is to provide an OpenGL context on as many
//!  platforms as possible.
//!
//! # Building a window
//!
//! There are two ways to create a window:
//!
//!  - Calling `Window::new()`.
//!  - Calling `let builder = WindowBuilder::new()` then `builder.build()`.
//!
//! The first way is the simpliest way and will give you default values.
//!
//! The second way allows you to customize the way your window and GL context
//!  will look and behave.

#[phase(plugin)] extern crate compile_msg;
#[phase(plugin)] extern crate gl_generator;
extern crate libc;

pub use events::*;

use std::default::Default;

#[cfg(target_os = "windows")]
use win32 as winimpl;
#[cfg(target_os = "linux")]
use x11 as winimpl;
#[cfg(target_os = "macos")]
use osx as winimpl;
#[cfg(target_os = "android")]
use android as winimpl;

#[cfg(target_os = "windows")]
mod win32;
#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "macos")]
mod osx;
#[cfg(target_os = "android")]
mod android;

mod events;

#[cfg(not(target_os = "windows"), not(target_os = "linux"), not(target_os = "macos"), not(target_os = "android"))]
compile_error!("Only the `windows`, `linux` and `macos` platforms are supported")

/// Identifier for a monitor.
pub struct MonitorID(winimpl::MonitorID);

/// Object that allows you to build windows.
pub struct WindowBuilder {
    dimensions: Option<(uint, uint)>,
    title: String,
    monitor: Option<winimpl::MonitorID>,
    gl_version: Option<(uint, uint)>,
    is_fullscreen: bool,
}

impl WindowBuilder {
    /// Initializes a new `WindowBuilder` with default values.
    pub fn new() -> WindowBuilder {
        WindowBuilder {
            dimensions: None,
            title: "gl-init-rs window".to_string(),
            monitor: None,
            gl_version: None,
            is_fullscreen: false,
        }
    }

    /// Requests the window to be of specific dimensions.
    ///
    /// Width and height are in pixels.
    pub fn with_dimensions(mut self, width: uint, height: uint) -> WindowBuilder {
        self.dimensions = Some((width, height));
        self
    }

    /// Requests a specific title for the window.
    pub fn with_title(mut self, title: String) -> WindowBuilder {
        self.title = title;
        self
    }

    /// Requests fullscreen mode.
    ///
    /// If you don't specify dimensions for the window, it will match the monitor's.
    pub fn with_fullscreen(mut self, monitor: MonitorID) -> WindowBuilder {
        let MonitorID(monitor) = monitor;
        self.monitor = Some(monitor);
        self.is_fullscreen = true;
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
    pub fn build(mut self) -> Result<Window, String> {
        // resizing the window to the dimensions of the monitor when fullscreen
        if self.dimensions.is_none() && self.monitor.is_some() {
            self.dimensions = Some(self.monitor.as_ref().unwrap().get_dimensions())
        }

        // default dimensions
        if self.dimensions.is_none() {
            self.dimensions = Some((1024, 768));
        }

        // building
        winimpl::Window::new(self).map(|w| Window { window: w })
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
    pub fn set_position(&self, x: int, y: int) {
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
    pub unsafe fn make_current(&self) {
        self.window.make_current()
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

    /// Returns the number of pixels currently displayed on the monitor.
    pub fn get_dimensions(&self) -> (uint, uint) {
        let &MonitorID(ref id) = self;
        id.get_dimensions()
    }
}
