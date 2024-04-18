use objc2::{extern_class, mutability, ClassType};
use objc2_foundation::NSObject;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UICoordinateSpace;

    unsafe impl ClassType for UICoordinateSpace {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);
