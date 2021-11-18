// Welcome to the util module, where we try to keep you from shooting yourself in the foot.
// *results may vary

mod atom;
mod client_msg;
mod cursor;
mod geometry;
mod hint;
mod icon;
mod input;
mod queue;
mod randr;
mod window_property;
mod wm;

pub use self::{
    atom::*, client_msg::*, geometry::*, hint::*, icon::*, input::*, queue::*, randr::*,
    window_property::*, wm::*,
};

use std::{
    mem::{self},
    ptr,
};

use super::{ffi, XConnection};
use xcb_dl_util::error::XcbError;
use xcb_dl_util::void::{XcbPendingCommand, XcbPendingCommands};
use xcb_dl_util::xcb_box::XcbBox;

pub fn maybe_change<T: PartialEq>(field: &mut Option<T>, value: T) -> bool {
    let wrapped = Some(value);
    if *field != wrapped {
        *field = wrapped;
        true
    } else {
        false
    }
}

pub fn fp1616_to_f64(x: ffi::xcb_input_fp1616_t) -> f64 {
    (x as f64 * 1.0) / ((1 << 16) as f64)
}

pub fn fp3232_to_f64(x: ffi::xcb_input_fp3232_t) -> f64 {
    x.integral as f64 + (x.frac as f64) / ((1u64 << 32) as f64)
}

impl XConnection {
    pub fn check_cookie(&self, cookie: ffi::xcb_void_cookie_t) -> Result<(), XcbError> {
        unsafe { self.errors.check_cookie(&self.xcb, cookie) }
    }

    pub unsafe fn check<T>(
        &self,
        val: *mut T,
        err: *mut ffi::xcb_generic_error_t,
    ) -> Result<XcbBox<T>, XcbError> {
        self.errors.check(&self.xcb, val, err)
    }

    pub fn check_pending1(&self, pending: XcbPendingCommand) -> Result<(), XcbError> {
        unsafe { pending.check(&self.xcb, &self.errors) }
    }

    pub fn check_pending(&self, pending: XcbPendingCommands) -> Result<(), XcbError> {
        unsafe { pending.check(&self.xcb, &self.errors) }
    }

    pub fn discard(&self, pending: XcbPendingCommand) {
        unsafe { pending.discard(&self.xcb, self.c) }
    }

    pub fn flush(&self) -> Result<(), XcbError> {
        unsafe {
            if self.xcb.xcb_flush(self.c) == 0 {
                self.errors.check_connection(&self.xcb)
            } else {
                Ok(())
            }
        }
    }

    pub fn generate_id(&self) -> u32 {
        unsafe { self.xcb.xcb_generate_id(self.c) }
    }
}
