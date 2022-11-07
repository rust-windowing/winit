use objc2::foundation::NSObject;
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

extern_class!(
    /// An object that stores color data and sometimes opacity (alpha value).
    ///
    /// <https://developer.apple.com/documentation/appkit/nscolor?language=objc>
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSColor;

    unsafe impl ClassType for NSColor {
        type Super = NSObject;
    }
);

// SAFETY: Documentation clearly states:
// > Color objects are immutable and thread-safe
unsafe impl Send for NSColor {}
unsafe impl Sync for NSColor {}

extern_methods!(
    unsafe impl NSColor {
        pub fn clear() -> Id<Self, Shared> {
            unsafe { msg_send_id![Self::class(), clearColor] }
        }
    }
);
