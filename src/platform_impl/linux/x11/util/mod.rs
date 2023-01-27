// Welcome to the util module, where we try to keep you from shooting yourself in the foot.
// *results may vary

mod atom;
mod client_msg;
mod cursor;
mod format;
mod geometry;
mod hint;
mod icon;
mod input;
pub mod keys;
mod memory;
pub mod modifiers;
mod randr;
mod window_property;
mod wm;

pub use self::{
    atom::*, client_msg::*, format::*, geometry::*, hint::*, icon::*, input::*, randr::*,
    window_property::*, wm::*,
};

pub(crate) use self::memory::*;

use std::{
    mem::{self, MaybeUninit},
    ops::BitAnd,
    os::raw::*,
    ptr,
};

use super::{ffi, XConnection, XError};

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

#[must_use = "This request was made asynchronously, and is still in the output buffer. You must explicitly choose to either `.flush()` (empty the output buffer, sending the request now) or `.queue()` (wait to send the request, allowing you to continue to add more requests without additional round-trips). For more information, see the documentation for `util::flush_requests`."]
pub(crate) struct Flusher<'a> {
    xconn: &'a XConnection,
}

impl<'a> Flusher<'a> {
    pub fn new(xconn: &'a XConnection) -> Self {
        Flusher { xconn }
    }

    // "I want this request sent now!"
    pub fn flush(self) -> Result<(), XError> {
        self.xconn.flush_requests()
    }

    // "I want the response now too!"
    pub fn sync(self) -> Result<(), XError> {
        self.xconn.sync_with_server()
    }

    // "I'm aware that this request hasn't been sent, and I'm okay with waiting."
    pub fn queue(self) {}
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
