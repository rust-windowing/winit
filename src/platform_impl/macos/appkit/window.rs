use objc2::foundation::NSObject;
use objc2::{extern_class, ClassType};

use super::NSResponder;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSWindow;

    unsafe impl ClassType for NSWindow {
        #[inherits(NSObject)]
        type Super = NSResponder;
    }
);
