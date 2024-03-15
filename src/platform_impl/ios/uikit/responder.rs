use icrate::Foundation::NSObject;
use objc2::{extern_class, mutability, ClassType, extern_methods};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIResponder;

    unsafe impl ClassType for UIResponder {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);
extern_methods!(
    unsafe impl UIResponder {
        // These are methods from UIResponder
        #[method(becomeFirstResponder)]
        pub fn become_first_responder(&self) -> bool;

        #[method(resignFirstResponder)]
        pub fn resign_first_responder(&self) -> bool;
    }
);
