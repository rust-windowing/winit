use objc2::foundation::{NSArray, NSObject};
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::NSWindow;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSWindowTabGroup;

    unsafe impl ClassType for NSWindowTabGroup {
        type Super = NSObject;
    }
);

extern_methods!(
    unsafe impl NSWindowTabGroup {
        #[sel(selectNextTab)]
        pub fn selectNextTab(&self);
        #[sel(selectPreviousTab)]
        pub fn selectPreviousTab(&self);
        pub fn tabbedWindows(&self) -> Id<NSArray<NSWindow, Shared>, Shared> {
            unsafe { msg_send_id![self, windows] }
        }
        #[sel(setSelectedWindow:)]
        pub fn setSelectedWindow(&self, window: &NSWindow);
    }
);
