use std::ops::{Deref, DerefMut};

use super::*;

pub(crate) struct XSmartPointer<'a, T> {
    xconn: &'a XConnection,
    pub ptr: *mut T,
}

impl<'a, T> XSmartPointer<'a, T> {
    // You're responsible for only passing things to this that should be XFree'd.
    // Returns None if ptr is null.
    pub fn new(xconn: &'a XConnection, ptr: *mut T) -> Option<Self> {
        if !ptr.is_null() {
            Some(XSmartPointer { xconn, ptr })
        } else {
            None
        }
    }
}

impl<'a, T> Deref for XSmartPointer<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<'a, T> DerefMut for XSmartPointer<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

impl<'a, T> Drop for XSmartPointer<'a, T> {
    fn drop(&mut self) {
        unsafe {
            (self.xconn.xlib.XFree)(self.ptr as *mut _);
        }
    }
}

impl XConnection {
    pub fn alloc_class_hint(&self) -> XSmartPointer<'_, ffi::XClassHint> {
        XSmartPointer::new(self, unsafe { (self.xlib.XAllocClassHint)() })
            .expect("`XAllocClassHint` returned null; out of memory")
    }

    pub fn alloc_size_hints(&self) -> XSmartPointer<'_, ffi::XSizeHints> {
        XSmartPointer::new(self, unsafe { (self.xlib.XAllocSizeHints)() })
            .expect("`XAllocSizeHints` returned null; out of memory")
    }

    pub fn alloc_wm_hints(&self) -> XSmartPointer<'_, ffi::XWMHints> {
        XSmartPointer::new(self, unsafe { (self.xlib.XAllocWMHints)() })
            .expect("`XAllocWMHints` returned null; out of memory")
    }
}
