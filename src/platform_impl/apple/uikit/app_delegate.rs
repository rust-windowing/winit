use objc2::{declare_class, mutability, ClassType, DeclaredClass};
use objc2_foundation::NSObject;
use objc2_ui_kit::UIApplication;

declare_class!(
    pub struct AppDelegate;

    unsafe impl ClassType for AppDelegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitApplicationDelegate";
    }

    impl DeclaredClass for AppDelegate {}

    // UIApplicationDelegate protocol
    unsafe impl AppDelegate {
        #[method(application:didFinishLaunchingWithOptions:)]
        fn did_finish_launching(&self, _application: &UIApplication, _: *mut NSObject) -> bool {
            true
        }
    }
);
