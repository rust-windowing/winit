use objc2::foundation::NSObject;
use objc2::{extern_class, extern_methods, ClassType};

use super::{NSResponder, NSView};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSControl;

    unsafe impl ClassType for NSControl {
        #[inherits(NSResponder, NSObject)]
        type Super = NSView;
    }
);

extern_methods!(
    unsafe impl NSControl {
        #[sel(setEnabled:)]
        pub fn setEnabled(&self, enabled: bool);

        #[sel(isEnabled)]
        pub fn isEnabled(&self) -> bool;
    }
);
