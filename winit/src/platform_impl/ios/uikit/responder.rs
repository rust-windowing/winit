use icrate::Foundation::NSObject;
use objc2::{extern_class, mutability, ClassType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIResponder;

    unsafe impl ClassType for UIResponder {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);
