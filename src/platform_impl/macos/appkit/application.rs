use objc2::foundation::NSObject;
use objc2::{extern_class, ClassType};

use super::NSResponder;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSApplication;

    unsafe impl ClassType for NSApplication {
        #[inherits(NSObject)]
        type Super = NSResponder;
    }
);
