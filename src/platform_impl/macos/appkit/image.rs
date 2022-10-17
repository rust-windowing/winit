use objc2::foundation::{NSData, NSObject, NSSize, NSString};
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send, msg_send_id, ClassType};

use super::NSBitmapImageRep;

extern_class!(
    // TODO: Can this be mutable?
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSImage;

    unsafe impl ClassType for NSImage {
        type Super = NSObject;
    }
);

// Documented Thread-Unsafe, but:
// > One thread can create an NSImage object, draw to the image buffer,
// > and pass it off to the main thread for drawing. The underlying image
// > cache is shared among all threads.
// <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html#//apple_ref/doc/uid/10000057i-CH12-126728>
//
// So really only unsafe to mutate on several threads.
unsafe impl Send for NSImage {}
unsafe impl Sync for NSImage {}

extern_methods!(
    unsafe impl NSImage {
        pub fn new_by_referencing_file(path: &NSString) -> Id<Self, Shared> {
            let this = unsafe { msg_send_id![Self::class(), alloc] };
            unsafe { msg_send_id![this, initByReferencingFile: path] }
        }

        pub fn new_with_data(data: &NSData) -> Id<Self, Shared> {
            let this = unsafe { msg_send_id![Self::class(), alloc] };
            unsafe { msg_send_id![this, initWithData: data] }
        }

        pub fn init_with_size(size: &NSSize) -> Id<Self, Shared> {
            let this = unsafe { msg_send_id![Self::class(), alloc] };
            unsafe { msg_send_id![this, initWithSize: size] }
        }

        pub fn add_representation(&self, representation: &NSBitmapImageRep) {
            unsafe { msg_send![self, addRepresentation: representation] }
        }
    }
);

extern_class!(
    /// <https://developer.apple.com/documentation/appkit/nsimagerep?language=objc>
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSImageRep;

    unsafe impl ClassType for NSImageRep {
        type Super = NSObject;
    }
);
