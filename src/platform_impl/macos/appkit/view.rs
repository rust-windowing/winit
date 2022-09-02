use objc2::foundation::{NSObject, NSRect};
use objc2::{extern_class, extern_methods, ClassType};

use super::{NSCursor, NSResponder};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSView;

    unsafe impl ClassType for NSView {
        #[inherits(NSObject)]
        type Super = NSResponder;
    }
);

extern_methods!(
    /// Getter methods
    unsafe impl NSView {
        #[sel(bounds)]
        pub fn bounds(&self) -> NSRect;
    }

    unsafe impl NSView {
        #[sel(addCursorRect:cursor:)]
        // NSCursor safe to take by shared reference since it is already immutable
        pub fn addCursorRect(&self, rect: NSRect, cursor: &NSCursor);
    }
);
