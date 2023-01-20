use std::ffi::c_void;
use std::ops::BitAnd;
use std::ptr;

use objc2::ffi::NSUInteger;
use objc2::foundation::{NSObject, NSRect};
use objc2::rc::{Id, Shared};
use objc2::runtime::Object;
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSTrackingArea;

    unsafe impl ClassType for NSTrackingArea {
        type Super = NSObject;
    }
);

extern_methods!(
    unsafe impl NSTrackingArea {
        unsafe fn inner_initWithRect(
            rect: NSRect,
            options: NSUInteger,
            owner: &Object,
            user_info: *mut c_void,
        ) -> Option<Id<NSTrackingArea, Shared>> {
            unsafe {
                let cls = msg_send_id![Self::class(), alloc];
                msg_send_id![
                    cls,
                    initWithRect: rect,
                    options: options,
                    owner: owner,
                    userInfo: user_info
                ]
            }
        }

        pub fn initWithRect(
            rect: NSRect,
            options: NSTrackingAreaOptions,
            owner: &Object,
        ) -> Option<Id<NSTrackingArea, Shared>> {
            if !options.are_valid() {
                return None;
            }
            //SAFETY: Returns none if options are invalid. userInfo is NULL, so it is guaranteed to be valid.
            unsafe { Self::inner_initWithRect(rect, options.bits, owner, ptr::null_mut()) }
        }
    }
);

// See https://developer.apple.com/documentation/appkit/nstrackingareaoptions?language=objc
bitflags! {
    pub(crate) struct NSTrackingAreaOptions: NSUInteger {
        const NSTrackingMouseEnteredAndExited = 1 << 0;
        const NSTrackingMouseMoved = 1 << 1;
        const NSTrackingCursorUpdate = 1 << 2;

        const NSTrackingActiveWhenFirstResponder = 1 << 4;
        const NSTrackingActiveInKeyWindow = 1 << 5;
        const NSTrackingActiveInActiveApp = 1 << 6;
        const NSTrackingActiveAlways = 1 << 7;

        const NSTrackingAssumeInside = 1 << 8;
        const NSTrackingInVisibleRect = 1 << 9;
        const NSTrackingEnabledDuringMouseDrag = 1 << 10;
    }
}

impl NSTrackingAreaOptions {
    pub fn are_valid(&self) -> bool {
        //ensure that exactly one active constant and at least one tracking-type constant are selected
        self.bitand(
            NSTrackingAreaOptions::NSTrackingMouseEnteredAndExited
                | NSTrackingAreaOptions::NSTrackingMouseMoved
                | NSTrackingAreaOptions::NSTrackingCursorUpdate,
        )
        .bits
            > 0
            && self
                .bitand(
                    NSTrackingAreaOptions::NSTrackingActiveAlways
                        | NSTrackingAreaOptions::NSTrackingActiveInActiveApp
                        | NSTrackingAreaOptions::NSTrackingActiveInKeyWindow
                        | NSTrackingAreaOptions::NSTrackingActiveWhenFirstResponder,
                )
                .bits
                .is_power_of_two()
    }
}
