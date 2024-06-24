use objc2::{declare_class, mutability, ClassType, DeclaredClass};
use objc2_foundation::NSObject;

declare_class!(
    pub struct AppDelegate;

    unsafe impl ClassType for AppDelegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitApplicationDelegate";
    }

    impl DeclaredClass for AppDelegate {}
);
