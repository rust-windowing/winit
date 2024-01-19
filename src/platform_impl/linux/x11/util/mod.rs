// Welcome to the util module, where we try to keep you from shooting yourself in the foot.
// *results may vary

mod client_msg;
mod cursor;
mod geometry;
mod hint;
mod icon;
mod input;
pub mod keys;
pub(crate) mod memory;
mod randr;
mod window_property;
mod wm;

pub use self::{cursor::*, geometry::*, hint::*, input::*, window_property::*, wm::*};

use std::{
    mem::{self, MaybeUninit},
    ops::BitAnd,
    os::raw::*,
};

use super::{atoms::*, ffi, VoidCookie, X11Error, XConnection, XError};
use x11rb::protocol::xproto::{self, ConnectionExt as _};

pub fn maybe_change<T: PartialEq>(field: &mut Option<T>, value: T) -> bool {
    let wrapped = Some(value);
    if *field != wrapped {
        *field = wrapped;
        true
    } else {
        false
    }
}

pub fn has_flag<T>(bitset: T, flag: T) -> bool
where
    T: Copy + PartialEq + BitAnd<T, Output = T>,
{
    bitset & flag == flag
}

impl XConnection {
    // This is impoartant, so pay attention!
    // Xlib has an output buffer, and tries to hide the async nature of X from you.
    // This buffer contains the requests you make, and is flushed under various circumstances:
    // 1. `XPending`, `XNextEvent`, and `XWindowEvent` flush "as needed"
    // 2. `XFlush` explicitly flushes
    // 3. `XSync` flushes and blocks until all requests are responded to
    // 4. Calls that have a return dependent on a response (i.e. `XGetWindowProperty`) sync internally.
    //    When in doubt, check the X11 source; if a function calls `_XReply`, it flushes and waits.
    // All util functions that abstract an async function will return a `Flusher`.
    pub fn flush_requests(&self) -> Result<(), XError> {
        unsafe { (self.xlib.XFlush)(self.display) };
        //println!("XFlush");
        // This isn't necessarily a useful time to check for errors (since our request hasn't
        // necessarily been processed yet)
        self.check_errors()
    }

    pub fn sync_with_server(&self) -> Result<(), XError> {
        unsafe { (self.xlib.XSync)(self.display, ffi::False) };
        //println!("XSync");
        self.check_errors()
    }
}
