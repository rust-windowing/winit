use objc2::{extern_class, mutability, ClassType};
use objc2_foundation::NSObject;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIEvent;

    unsafe impl ClassType for UIEvent {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);
