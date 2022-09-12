use objc2::foundation::NSObject;
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::NSMenuItem;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSMenu;

    unsafe impl ClassType for NSMenu {
        type Super = NSObject;
    }
);

extern_methods!(
    unsafe impl NSMenu {
        pub fn new() -> Id<Self, Shared> {
            unsafe { msg_send_id![Self::class(), new] }
        }

        #[sel(addItem:)]
        pub fn addItem(&self, item: &NSMenuItem);
    }
);
