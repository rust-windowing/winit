use icrate::AppKit::NSImage;
use icrate::Foundation::{NSObject, NSPoint};
use objc2::rc::Id;
use objc2::{extern_class, extern_methods, msg_send_id, mutability, ClassType};

extern_class!(
    /// <https://developer.apple.com/documentation/appkit/nscursor?language=objc>
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSCursor;

    unsafe impl ClassType for NSCursor {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

// SAFETY: NSCursor is immutable, stated here:
// https://developer.apple.com/documentation/appkit/nscursor/1527062-image?language=objc
unsafe impl Send for NSCursor {}
unsafe impl Sync for NSCursor {}

macro_rules! def_cursor {
    {$(
        $(#[$($m:meta)*])*
        pub fn $name:ident();
    )*} => {$(
        $(#[$($m)*])*
        pub fn $name() -> Id<Self> {
            unsafe { msg_send_id![Self::class(), $name] }
        }
    )*};
}

extern_methods!(
    /// Documented cursors
    unsafe impl NSCursor {
        def_cursor!(
            pub fn arrowCursor();
            pub fn pointingHandCursor();
            pub fn openHandCursor();
            pub fn closedHandCursor();
            pub fn IBeamCursor();
            pub fn IBeamCursorForVerticalLayout();
            pub fn dragCopyCursor();
            pub fn dragLinkCursor();
            pub fn operationNotAllowedCursor();
            pub fn contextualMenuCursor();
            pub fn crosshairCursor();
            pub fn resizeRightCursor();
            pub fn resizeUpCursor();
            pub fn resizeLeftCursor();
            pub fn resizeDownCursor();
            pub fn resizeLeftRightCursor();
            pub fn resizeUpDownCursor();
        );

        // Creating cursors should be thread-safe, though using them for anything probably isn't.
        pub fn new(image: &NSImage, hotSpot: NSPoint) -> Id<Self> {
            unsafe { msg_send_id![Self::alloc(), initWithImage: image, hotSpot: hotSpot] }
        }
    }
);
