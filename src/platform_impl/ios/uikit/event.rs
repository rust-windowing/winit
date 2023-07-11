use objc2::foundation::NSObject;
use objc2::{extern_class, ClassType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIEvent;

    unsafe impl ClassType for UIEvent {
        type Super = NSObject;
    }
);
