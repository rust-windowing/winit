use std::ptr;
use std::sync::Arc;
use std::os::raw::{c_short, c_void};

use super::{ffi, util, XConnection, XError};

#[derive(Debug)]
pub enum ImeContextCreationError {
    XError(XError),
    Null,
}

unsafe fn create_pre_edit_attr<'a>(
    xconn: &'a Arc<XConnection>,
    ic_spot: &'a ffi::XPoint,
) -> util::XSmartPointer<'a, c_void> {
    util::XSmartPointer::new(
        xconn,
        (xconn.xlib.XVaCreateNestedList)(
            0,
            ffi::XNSpotLocation_0.as_ptr() as *const _,
            ic_spot,
            ptr::null_mut::<()>(),
        ),
    ).expect("XVaCreateNestedList returned NULL")
}

// WARNING: this struct doesn't destroy its XIC resource when dropped.
// This is intentional, as it doesn't have enough information to know whether or not the context
// still exists on the server. Since `ImeInner` has that awareness, destruction must be handled
// through `ImeInner`.
#[derive(Debug)]
pub struct ImeContext {
    pub ic: ffi::XIC,
    pub ic_spot: ffi::XPoint,
}

impl ImeContext {
    pub unsafe fn new(
        xconn: &Arc<XConnection>,
        im: ffi::XIM,
        window: ffi::Window,
        ic_spot: Option<ffi::XPoint>,
    ) -> Result<Self, ImeContextCreationError> {
        let ic = if let Some(ic_spot) = ic_spot {
            ImeContext::create_ic_with_spot(xconn, im, window, ic_spot)
        } else {
            ImeContext::create_ic(xconn, im, window)
        };

        let ic = ic.ok_or(ImeContextCreationError::Null)?;
        xconn.check_errors().map_err(ImeContextCreationError::XError)?;

        Ok(ImeContext {
            ic,
            ic_spot: ic_spot.unwrap_or_else(|| ffi::XPoint { x: 0, y: 0 }),
        })
    }

    unsafe fn create_ic(
        xconn: &Arc<XConnection>,
        im: ffi::XIM,
        window: ffi::Window,
    ) -> Option<ffi::XIC> {
        let ic = (xconn.xlib.XCreateIC)(
            im,
            ffi::XNInputStyle_0.as_ptr() as *const _,
            ffi::XIMPreeditNothing | ffi::XIMStatusNothing,
            ffi::XNClientWindow_0.as_ptr() as *const _,
            window,
            ptr::null_mut::<()>(),
        );
        if ic.is_null() {
            None
        } else {
            Some(ic)
        }
    }

    unsafe fn create_ic_with_spot(
        xconn: &Arc<XConnection>,
        im: ffi::XIM,
        window: ffi::Window,
        ic_spot: ffi::XPoint,
    ) -> Option<ffi::XIC> {
        let pre_edit_attr = create_pre_edit_attr(xconn, &ic_spot);
        let ic = (xconn.xlib.XCreateIC)(
            im,
            ffi::XNInputStyle_0.as_ptr() as *const _,
            ffi::XIMPreeditNothing | ffi::XIMStatusNothing,
            ffi::XNClientWindow_0.as_ptr() as *const _,
            window,
            ffi::XNPreeditAttributes_0.as_ptr() as *const _,
            pre_edit_attr.ptr,
            ptr::null_mut::<()>(),
        );
        if ic.is_null() {
            None
        } else {
            Some(ic)
        }
    }

    pub fn focus(&self, xconn: &Arc<XConnection>) -> Result<(), XError> {
        unsafe {
            (xconn.xlib.XSetICFocus)(self.ic);
        }
        xconn.check_errors()
    }

    pub fn unfocus(&self, xconn: &Arc<XConnection>) -> Result<(), XError> {
        unsafe {
            (xconn.xlib.XUnsetICFocus)(self.ic);
        }
        xconn.check_errors()
    }

    pub fn set_spot(&mut self, xconn: &Arc<XConnection>, x: c_short, y: c_short) {
        if self.ic_spot.x == x && self.ic_spot.y == y {
            return;
        }
        self.ic_spot = ffi::XPoint { x, y };

        unsafe {
            let pre_edit_attr = create_pre_edit_attr(xconn, &self.ic_spot);
            (xconn.xlib.XSetICValues)(
                self.ic,
                ffi::XNPreeditAttributes_0.as_ptr() as *const _,
                pre_edit_attr.ptr,
                ptr::null_mut::<()>(),
            );
        }
    }
}
