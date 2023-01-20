use objc2::foundation::{NSObject, NSPoint, NSRect};
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::{
    NSCursor, NSResponder, NSTextInputContext, NSTrackingArea, NSTrackingAreaOptions, NSWindow,
};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSView;

    unsafe impl ClassType for NSView {
        #[inherits(NSObject)]
        type Super = NSResponder;
    }
);

// Documented as "Main Thread Only".
// > generally thread safe, although operations on views such as creating,
// > resizing, and moving should happen on the main thread.
// <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/CocoaFundamentals/AddingBehaviortoaCocoaProgram/AddingBehaviorCocoa.html#//apple_ref/doc/uid/TP40002974-CH5-SW47>
//
// > If you want to use a thread to draw to a view, bracket all drawing code
// > between the lockFocusIfCanDraw and unlockFocus methods of NSView.
// <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html#//apple_ref/doc/uid/10000057i-CH12-123351-BBCFIIEB>

extern_methods!(
    /// Getter methods
    unsafe impl NSView {
        #[sel(frame)]
        pub fn frame(&self) -> NSRect;

        #[sel(bounds)]
        pub fn bounds(&self) -> NSRect;

        pub fn inputContext(
            &self,
            // _mtm: MainThreadMarker,
        ) -> Option<Id<NSTextInputContext, Shared>> {
            unsafe { msg_send_id![self, inputContext] }
        }

        #[sel(visibleRect)]
        pub fn visibleRect(&self) -> NSRect;

        #[sel(hasMarkedText)]
        pub fn hasMarkedText(&self) -> bool;

        #[sel(convertPoint:fromView:)]
        pub fn convertPoint_fromView(&self, point: NSPoint, view: Option<&NSView>) -> NSPoint;

        pub fn window(&self) -> Option<Id<NSWindow, Shared>> {
            unsafe { msg_send_id![self, window] }
        }
    }

    unsafe impl NSView {
        #[sel(setWantsBestResolutionOpenGLSurface:)]
        pub fn setWantsBestResolutionOpenGLSurface(&self, value: bool);

        #[sel(setWantsLayer:)]
        pub fn setWantsLayer(&self, wants_layer: bool);

        #[sel(setPostsFrameChangedNotifications:)]
        pub fn setPostsFrameChangedNotifications(&mut self, value: bool);

        #[sel(addTrackingArea:)]
        pub fn addTrackingArea(&self, area: &NSTrackingArea);

        #[sel(removeTrackingArea:)]
        pub fn removeTrackingArea(&self, area: &NSTrackingArea);

        pub fn init_and_add_tracking_area(
            &self,
            options: NSTrackingAreaOptions,
            rect: NSRect,
        ) -> Id<NSTrackingArea, Shared> {
            let tracking_area = NSTrackingArea::initWithRect(rect, options, self)
                .expect("failed to create tracking area");
            self.addTrackingArea(&tracking_area);
            tracking_area
        }

        #[sel(addCursorRect:cursor:)]
        // NSCursor safe to take by shared reference since it is already immutable
        pub fn addCursorRect(&self, rect: NSRect, cursor: &NSCursor);

        #[sel(setHidden:)]
        pub fn setHidden(&self, hidden: bool);
    }
);
