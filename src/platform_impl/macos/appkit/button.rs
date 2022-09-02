use objc2::foundation::NSObject;
use objc2::{extern_class, ClassType};

use super::{NSControl, NSResponder, NSView};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSButton;

    unsafe impl ClassType for NSButton {
        #[inherits(NSView, NSResponder, NSObject)]
        type Super = NSControl;
    }
);
