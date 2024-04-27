use objc2::encode::{Encode, Encoding};
use objc2::{extern_class, extern_methods, mutability, ClassType};
use objc2_foundation::{NSInteger, NSObject};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UITraitCollection;

    unsafe impl ClassType for UITraitCollection {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UITraitCollection {
        #[method(forceTouchCapability)]
        pub fn forceTouchCapability(&self) -> UIForceTouchCapability;
    }
);

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UIForceTouchCapability {
    Unknown = 0,
    Unavailable,
    Available,
}

unsafe impl Encode for UIForceTouchCapability {
    const ENCODING: Encoding = NSInteger::ENCODING;
}
