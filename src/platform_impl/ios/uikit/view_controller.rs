use icrate::Foundation::{NSObject, NSUInteger};
use objc2::encode::{Encode, Encoding};
use objc2::rc::Id;
use objc2::{extern_class, extern_methods, msg_send_id, mutability, ClassType};

use super::{UIResponder, UIView};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIViewController;

    unsafe impl ClassType for UIViewController {
        #[inherits(NSObject)]
        type Super = UIResponder;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIViewController {
        #[method(attemptRotationToDeviceOrientation)]
        pub fn attemptRotationToDeviceOrientation();

        #[method(setNeedsStatusBarAppearanceUpdate)]
        pub fn setNeedsStatusBarAppearanceUpdate(&self);

        #[method(setNeedsUpdateOfHomeIndicatorAutoHidden)]
        pub fn setNeedsUpdateOfHomeIndicatorAutoHidden(&self);

        #[method(setNeedsUpdateOfScreenEdgesDeferringSystemGestures)]
        pub fn setNeedsUpdateOfScreenEdgesDeferringSystemGestures(&self);

        pub fn view(&self) -> Option<Id<UIView>> {
            unsafe { msg_send_id![self, view] }
        }

        #[method(setView:)]
        pub fn setView(&self, view: Option<&UIView>);
    }
);

bitflags! {
    #[derive(Clone, Copy)]
    pub struct UIInterfaceOrientationMask: NSUInteger {
        const Portrait = 1 << 1;
        const PortraitUpsideDown = 1 << 2;
        const LandscapeRight = 1 << 3;
        const LandscapeLeft = 1 << 4;
        const Landscape = Self::LandscapeLeft.bits() | Self::LandscapeRight.bits();
        const AllButUpsideDown = Self::Landscape.bits() | Self::Portrait.bits();
        const All = Self::AllButUpsideDown.bits() | Self::PortraitUpsideDown.bits();
    }
}

unsafe impl Encode for UIInterfaceOrientationMask {
    const ENCODING: Encoding = NSUInteger::ENCODING;
}
