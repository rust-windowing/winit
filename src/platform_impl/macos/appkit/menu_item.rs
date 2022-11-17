use objc2::foundation::{NSObject, NSString};
use objc2::rc::{Id, Shared};
use objc2::runtime::Sel;
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::{NSEventModifierFlags, NSMenu};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSMenuItem;

    unsafe impl ClassType for NSMenuItem {
        type Super = NSObject;
    }
);

extern_methods!(
    unsafe impl NSMenuItem {
        pub fn new() -> Id<Self, Shared> {
            unsafe { msg_send_id![Self::class(), new] }
        }

        pub fn newWithTitle(
            title: &NSString,
            action: Sel,
            key_equivalent: &NSString,
        ) -> Id<Self, Shared> {
            unsafe {
                msg_send_id![
                    msg_send_id![Self::class(), alloc],
                    initWithTitle: title,
                    action: action,
                    keyEquivalent: key_equivalent,
                ]
            }
        }

        pub fn separatorItem() -> Id<Self, Shared> {
            unsafe { msg_send_id![Self::class(), separatorItem] }
        }

        #[sel(setKeyEquivalentModifierMask:)]
        pub fn setKeyEquivalentModifierMask(&self, mask: NSEventModifierFlags);

        #[sel(setSubmenu:)]
        pub fn setSubmenu(&self, submenu: &NSMenu);
    }
);
