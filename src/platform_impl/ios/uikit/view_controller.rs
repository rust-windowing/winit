use objc2::encode::{Encode, Encoding};
use objc2::foundation::{NSObject, NSUInteger};
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::{UIResponder, UIView};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIViewController;

    unsafe impl ClassType for UIViewController {
        #[inherits(NSObject)]
        type Super = UIResponder;
    }
);

extern_methods!(
    unsafe impl UIViewController {
        #[sel(attemptRotationToDeviceOrientation)]
        pub fn attemptRotationToDeviceOrientation();

        #[sel(setNeedsStatusBarAppearanceUpdate)]
        pub fn setNeedsStatusBarAppearanceUpdate(&self);

        #[sel(setNeedsUpdateOfHomeIndicatorAutoHidden)]
        pub fn setNeedsUpdateOfHomeIndicatorAutoHidden(&self);

        #[sel(setNeedsUpdateOfScreenEdgesDeferringSystemGestures)]
        pub fn setNeedsUpdateOfScreenEdgesDeferringSystemGestures(&self);

        pub fn view(&self) -> Option<Id<UIView, Shared>> {
            unsafe { msg_send_id![self, view] }
        }

        #[sel(setView:)]
        pub fn setView(&self, view: Option<&UIView>);
    }
);

bitflags! {
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
