use objc2::foundation::{MainThreadMarker, NSObject};
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::super::ffi::UIUserInterfaceIdiom;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIDevice;

    unsafe impl ClassType for UIDevice {
        type Super = NSObject;
    }
);

extern_methods!(
    unsafe impl UIDevice {
        pub fn current(_mtm: MainThreadMarker) -> Id<Self, Shared> {
            unsafe { msg_send_id![Self::class(), currentDevice] }
        }

        #[sel(userInterfaceIdiom)]
        pub fn userInterfaceIdiom(&self) -> UIUserInterfaceIdiom;
    }
);
