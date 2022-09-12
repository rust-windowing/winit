use objc2::foundation::NSObject;
use objc2::{extern_class, ClassType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIResponder;

    unsafe impl ClassType for UIResponder {
        type Super = NSObject;
    }
);
