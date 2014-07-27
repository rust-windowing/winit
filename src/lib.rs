#![feature(unsafe_destructor)]

extern crate libc;

pub use events::{Event, PositionChanged, SizeChanged, Closed, CursorPositionChanged, Focused};
pub use events::{Iconified, NeedRefresh, FramebufferSizeChanged};
pub use hints::{Hints, ClientAPI, Profile};

#[cfg(windows)]
use winimpl = win32;
#[cfg(unix)]
use winimpl = x11;

#[cfg(windows)]
mod win32;
#[cfg(unix)]
mod x11;

mod events;
mod hints;

pub struct Window {
    window: winimpl::Window,
    nosend: std::kinds::marker::NoSend,
}

impl Window {
    #[inline]
    pub fn new(dimensions: Option<(uint, uint)>, title: &str, hints: &Hints)
        -> Result<Window, String>
    {
        let win = try!(winimpl::Window::new(dimensions, title, hints));
        Ok(Window{
            window: win,
            nosend: std::kinds::marker::NoSend,
        })
    }

    /// Returns true if the window has been closed by the user.
    #[inline]
    pub fn should_close(&self) -> bool {
        self.window.should_close()
    }

    /// Modifies the title of the window.
    #[inline]
    pub fn set_title(&self, title: &str) {
        self.window.set_title(title)
    }

    #[inline]
    pub fn get_position(&self) -> (uint, uint) {
        self.window.get_position()
    }

    #[inline]
    pub fn set_position(&self, x: uint, y: uint) {
        self.window.set_position(x, y)
    }

    #[inline]
    pub fn get_size(&self) -> (uint, uint) {
        self.window.get_size()
    }

    #[inline]
    pub fn set_size(&self, x: uint, y: uint) {
        self.window.set_size(x, y)
    }

    // TODO: return iterator
    #[inline]
    pub fn poll_events(&self) -> Vec<Event> {
        self.window.poll_events()
    }

    // TODO: return iterator
    #[inline]
    pub fn wait_events(&self) -> Vec<Event> {
        self.window.wait_events()
    }

    #[inline]
    pub fn make_current(&self) {
        self.window.make_current()
    }

    #[inline]
    pub fn get_proc_address(&self, addr: &str) -> *const () {
        self.window.get_proc_address(addr)
    }

    #[inline]
    pub fn swap_buffers(&self) {
        self.window.swap_buffers()
    }
}
