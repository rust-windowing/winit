use std::ffi::c_void;
use std::ptr;

use objc2::foundation::{NSObject, NSRect};
use objc2::ffi::NSUInteger;
use objc2::runtime::Object;
use objc2::{extern_class, ClassType, extern_methods, msg_send_id};
use objc2::rc::{Id, Shared};

use super::{NSView};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSTrackingArea;

    unsafe impl ClassType for NSTrackingArea {
        type Super = NSObject;
    }
);

extern_methods!(
    unsafe impl NSTrackingArea {

        pub fn inner_initWithRect(
            rect: NSRect,
            options: NSUInteger,
            owner: &Object,
            user_info: *mut c_void,
        ) -> Option<Id<NSTrackingArea, Shared>> {
            unsafe {
                let cls = msg_send_id![Self::class(), alloc];
                msg_send_id![cls, initWithRect: rect, options: options, owner: owner, userInfo: user_info]
            }
        }

        pub fn initWithRect(
            rect: NSRect,
            options: NSUInteger,
            owner: &Object
        ) -> Option<Id<NSTrackingArea, Shared>> {
            //SAFETY: Return none if options are invalid. userInfo is NULL, so it is valid.
            if options & 0xF0 == 0 || options & 0x7 == 0 {
                return None
            }
            unsafe {
                Self::inner_initWithRect(rect, options, owner, ptr::null_mut())
            }
        }
    }
);

// See https://developer.apple.com/documentation/appkit/nstrackingareaoptions?language=objc
bitflags! {
    pub struct NSTrackingAreaOptions: NSUInteger {
        const NSTrackingMouseEnteredAndExited = 1 << 0;
        const NSTrackingMouseMoved = 1 << 2;
        const NSTrackingCursorUpdate = 1 << 3;
        const NSTrackingActiveWhenFirstResponder = 1 << 4;
        const NSTrackingActiveInKeyWindow = 1 << 5;
        const NSTrackingActiveInActiveApp = 1 << 6;
        const NSTrackingActiveAlways = 1 << 7;
        const NSTrackingAssumeInside = 1 << 8;
        const NSTrackingInVisibleRect = 1 << 9;
        const NSTrackingEnabledDuringMouseDrag = 1 << 10;
    }
}