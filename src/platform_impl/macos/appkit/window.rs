use objc2::foundation::{CGFloat, NSObject, NSRect, NSSize};
use objc2::rc::{Id, Shared};
use objc2::runtime::Object;
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::NSResponder;

extern_class!(
    /// Main-Thread-Only!
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSWindow;

    unsafe impl ClassType for NSWindow {
        #[inherits(NSObject)]
        type Super = NSResponder;
    }
);

// Documented as "Main Thread Only", but:
// > Thread safe in that you can create and manage them on a secondary thread.
// <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/CocoaFundamentals/AddingBehaviortoaCocoaProgram/AddingBehaviorCocoa.html#//apple_ref/doc/uid/TP40002974-CH5-SW47>
// <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html#//apple_ref/doc/uid/10000057i-CH12-123364>
//
// So could in theory be `Send`, and perhaps also `Sync` - but we would like
// interior mutability on windows, since that's just much easier, and in that
// case, they can't be!

extern_methods!(
    unsafe impl NSWindow {
        #[sel(frame)]
        pub fn frame(&self) -> NSRect;

        #[sel(backingScaleFactor)]
        pub fn backingScaleFactor(&self) -> CGFloat;

        #[sel(contentRectForFrameRect:)]
        pub fn contentRectForFrameRect(&self, windowFrame: NSRect) -> NSRect;

        #[sel(setContentSize:)]
        pub fn setContentSize(&self, contentSize: NSSize);

        #[sel(setMinSize:)]
        pub fn setMinSize(&self, minSize: NSSize);

        #[sel(setMaxSize:)]
        pub fn setMaxSize(&self, maxSize: NSSize);

        #[sel(setFrame:display:)]
        pub fn setFrame_display(&self, frameRect: NSRect, flag: bool);

        #[sel(setMovable:)]
        pub fn setMovable(&self, movable: bool);

        #[sel(miniaturize:)]
        pub fn miniaturize(&self, sender: Option<&Object>);

        #[sel(sender:)]
        pub fn deminiaturize(&self, sender: Option<&Object>);

        #[sel(selectNextKeyView:)]
        pub fn selectNextKeyView(&self, sender: Option<&Object>);

        #[sel(selectPreviousKeyView:)]
        pub fn selectPreviousKeyView(&self, sender: Option<&Object>);

        pub fn firstResponder(&self) -> Option<Id<NSResponder, Shared>> {
            unsafe { msg_send_id![self, firstResponder] }
        }
    }
);
