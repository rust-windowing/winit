use icrate::Foundation::{MainThreadMarker, NSObject};
use objc2::rc::Id;
use objc2::{extern_class, extern_methods, msg_send_id, mutability, ClassType};

use super::super::ffi::UIUserInterfaceIdiom;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIDevice;

    unsafe impl ClassType for UIDevice {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIDevice {
        pub fn current(_mtm: MainThreadMarker) -> Id<Self> {
            unsafe { msg_send_id![Self::class(), currentDevice] }
        }

        #[method(userInterfaceIdiom)]
        pub fn userInterfaceIdiom(&self) -> UIUserInterfaceIdiom;
    }
);
