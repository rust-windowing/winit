use objc2::foundation::{NSArray, NSObject};
use objc2::rc::Shared;
use objc2::{extern_class, extern_methods, ClassType};

use super::NSEvent;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSResponder;

    unsafe impl ClassType for NSResponder {
        type Super = NSObject;
    }
);

// Documented as "Thread-Unsafe".

extern_methods!(
    unsafe impl NSResponder {
        // TODO: Allow "immutably" on main thread
        #[sel(interpretKeyEvents:)]
        pub unsafe fn interpretKeyEvents(&mut self, events: &NSArray<NSEvent, Shared>);
    }
);
