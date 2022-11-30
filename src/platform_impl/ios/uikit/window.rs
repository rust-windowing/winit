use objc2::foundation::{CGRect, NSObject};
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::{UIResponder, UIScreen, UIView};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIWindow;

    unsafe impl ClassType for UIWindow {
        #[inherits(UIResponder, NSObject)]
        type Super = UIView;
    }
);

extern_methods!(
    unsafe impl UIWindow {
        pub fn screen(&self) -> Id<UIScreen, Shared> {
            unsafe { msg_send_id![self, screen] }
        }

        #[sel(setScreen:)]
        pub fn setScreen(&self, screen: &UIScreen);

        #[sel(setHidden:)]
        pub fn setHidden(&self, flag: bool);

        #[sel(setFrame:)]
        pub fn setFrame(&self, value: CGRect);

        #[sel(makeKeyAndVisible)]
        pub fn makeKeyAndVisible(&self);
    }
);
