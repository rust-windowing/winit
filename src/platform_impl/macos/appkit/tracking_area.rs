use std::ops::BitAnd;

use objc2::ffi::NSUInteger;
use objc2::foundation::{NSDictionary, NSObject, NSRect};
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
        pub fn initWithRect(
            rect: NSRect,
            options: NSTrackingAreaOptions,
            owner: Option<&Object>,
            user_info: Option<&NSDictionary<Object, Object>>,
        ) -> Option<Id<NSTrackingArea, Shared>> {
            if options.are_valid() {
                let cls = msg_send_id![Self::class(), alloc];
                msg_send_id![
                    cls,
                    initWithRect: rect,
                    options: options.bits,
                    owner: owner,
                    userInfo: user_info
                ]
            } else {
                None
            }
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
        //ensure that at least one tracking-type constant and exactly one active constant are specified
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
