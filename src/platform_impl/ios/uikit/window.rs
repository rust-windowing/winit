use icrate::Foundation::NSObject;
use objc2::rc::Id;
use objc2::{extern_class, extern_methods, msg_send_id, mutability, ClassType};

use super::{UIResponder, UIScreen, UIView};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIWindow;

    unsafe impl ClassType for UIWindow {
        #[inherits(UIResponder, NSObject)]
        type Super = UIView;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIWindow {
        pub fn screen(&self) -> Id<UIScreen> {
            unsafe { msg_send_id![self, screen] }
        }

        #[method(setScreen:)]
        pub fn setScreen(&self, screen: &UIScreen);

        #[method(setHidden:)]
        pub fn setHidden(&self, flag: bool);

        #[method(makeKeyAndVisible)]
        pub fn makeKeyAndVisible(&self);

        #[method(isKeyWindow)]
        pub fn isKeyWindow(&self) -> bool;
    }
);
