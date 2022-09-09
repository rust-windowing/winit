use objc2::foundation::NSObject;
use objc2::{extern_class, ClassType};

use super::{NSResponder, NSView};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSControl;

    unsafe impl ClassType for NSControl {
        #[inherits(NSResponder, NSObject)]
        type Super = NSView;
    }
);
