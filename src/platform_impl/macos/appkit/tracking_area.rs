use std::ffi::c_void;
use std::ptr;

use objc2::foundation::{NSObject, NSRect};
use objc2::ffi::NSUInteger;
use objc2::runtime::Object;
use objc2::{extern_class, ClassType, extern_methods, Encoding, Encode};

use super::{NSView};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSTrackingArea;

    unsafe impl ClassType for NSTrackingArea {
        #[inherits(NSObject)]
        type Super = NSView;
    }
);
unsafe impl Encode for NSTrackingArea {
    const ENCODING: Encoding = Encoding::Class;
}
extern_methods!(
    unsafe impl NSTrackingArea {
        #[sel(initWithRect:options:owner:userInfo:)]
        unsafe fn inner_initWithRect(
            options: NSUInteger,
            owner: &Object,
            rect: NSRect,
            user_info: *mut c_void,
        ) -> NSTrackingArea;

        pub fn initWithRect(&self,options: NSUInteger, rect: NSRect) -> Option<NSTrackingArea> {
            //SAFETY: Return none if options are invalid. userInfo is NULL, so it is valid.
            if options & 0xF0 == 0 || options & 0x7 == 0 {
                return None
            }
            Some(unsafe {
                Self::inner_initWithRect(options, self, rect, ptr::null_mut())
            })
        }
    }
);
