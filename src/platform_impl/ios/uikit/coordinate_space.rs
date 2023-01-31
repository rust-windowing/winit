use objc2::foundation::NSObject;
use objc2::{extern_class, ClassType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UICoordinateSpace;

    unsafe impl ClassType for UICoordinateSpace {
        type Super = NSObject;
    }
);
