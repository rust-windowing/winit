use std::ops::{Deref, DerefMut};

use super::*;

pub struct XSmartPointer<T> {
    pub ptr: *mut T,
}

impl<T> XSmartPointer<T> {
    // You're responsible for only passing things to this that should be XFree'd.
    // Returns None if ptr is null.
    pub fn new(ptr: *mut T) -> Option<Self> {
        if !ptr.is_null() {
            Some(XSmartPointer { ptr })
        } else {
            None
        }
    }
}

impl<T> Deref for XSmartPointer<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for XSmartPointer<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

impl<T> Drop for XSmartPointer<T> {
    fn drop(&mut self) {
        let xlib = syms!(XLIB);
        unsafe {
            (xlib.XFree)(self.ptr as *mut _);
        }
    }
}

impl XConnection {
    pub fn alloc_class_hint(&self) -> XSmartPointer<ffi::XClassHint> {
        let xlib = syms!(XLIB);
        XSmartPointer::new(unsafe { (xlib.XAllocClassHint)() })
            .expect("`XAllocClassHint` returned null; out of memory")
    }

    pub fn alloc_size_hints(&self) -> XSmartPointer<ffi::XSizeHints> {
        let xlib = syms!(XLIB);
        XSmartPointer::new(unsafe { (xlib.XAllocSizeHints)() })
            .expect("`XAllocSizeHints` returned null; out of memory")
    }

    pub fn alloc_wm_hints(&self) -> XSmartPointer<ffi::XWMHints> {
        let xlib = syms!(XLIB);
        XSmartPointer::new(unsafe { (xlib.XAllocWMHints)() })
            .expect("`XAllocWMHints` returned null; out of memory")
    }
}
