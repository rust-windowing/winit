use icrate::Foundation::{CGRect, MainThreadMarker, NSArray, NSObject};
use objc2::rc::Id;
use objc2::{extern_class, extern_methods, msg_send_id, mutability, ClassType};

use super::{UIResponder, UIWindow};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIApplication;

    unsafe impl ClassType for UIApplication {
        #[inherits(NSObject)]
        type Super = UIResponder;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIApplication {
        pub fn shared(_mtm: MainThreadMarker) -> Option<Id<Self>> {
            unsafe { msg_send_id![Self::class(), sharedApplication] }
        }

        pub fn windows(&self) -> Id<NSArray<UIWindow>> {
            unsafe { msg_send_id![self, windows] }
        }

        #[method(statusBarFrame)]
        pub fn statusBarFrame(&self) -> CGRect;
    }
);
