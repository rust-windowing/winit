use objc2::foundation::NSObject;
use objc2::{extern_class, ClassType};

use super::UIResponder;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIWindow;

    unsafe impl ClassType for UIWindow {
        #[inherits(NSObject)]
        type Super = UIResponder;
    }
);
