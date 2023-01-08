use objc2::foundation::{CGRect, MainThreadMarker, NSArray, NSObject};
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::{UIResponder, UIWindow};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIApplication;

    unsafe impl ClassType for UIApplication {
        #[inherits(NSObject)]
        type Super = UIResponder;
    }
);

extern_methods!(
    unsafe impl UIApplication {
        pub fn shared(_mtm: MainThreadMarker) -> Option<Id<Self, Shared>> {
            unsafe { msg_send_id![Self::class(), sharedApplication] }
        }

        pub fn windows(&self) -> Id<NSArray<UIWindow, Shared>, Shared> {
            unsafe { msg_send_id![self, windows] }
        }

        #[sel(statusBarFrame)]
        pub fn statusBarFrame(&self) -> CGRect;
    }
);
