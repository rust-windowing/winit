use objc2::encode::{Encode, Encoding};
use objc2::foundation::{CGFloat, CGRect, NSObject};
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::{UICoordinateSpace, UIResponder, UIViewController};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIView;

    unsafe impl ClassType for UIView {
        #[inherits(NSObject)]
        type Super = UIResponder;
    }
);

extern_methods!(
    unsafe impl UIView {
        #[sel(bounds)]
        pub fn bounds(&self) -> CGRect;

        #[sel(setBounds:)]
        pub fn setBounds(&self, value: CGRect);

        #[sel(frame)]
        pub fn frame(&self) -> CGRect;

        #[sel(setFrame:)]
        pub fn setFrame(&self, value: CGRect);

        #[sel(contentScaleFactor)]
        pub fn contentScaleFactor(&self) -> CGFloat;

        #[sel(setContentScaleFactor:)]
        pub fn setContentScaleFactor(&self, val: CGFloat);

        #[sel(setMultipleTouchEnabled:)]
        pub fn setMultipleTouchEnabled(&self, val: bool);

        pub fn rootViewController(&self) -> Option<Id<UIViewController, Shared>> {
            unsafe { msg_send_id![self, rootViewController] }
        }

        #[sel(setRootViewController:)]
        pub fn setRootViewController(&self, rootViewController: Option<&UIViewController>);

        #[sel(convertRect:toCoordinateSpace:)]
        pub fn convertRect_toCoordinateSpace(
            &self,
            rect: CGRect,
            coordinateSpace: &UICoordinateSpace,
        ) -> CGRect;

        #[sel(convertRect:fromCoordinateSpace:)]
        pub fn convertRect_fromCoordinateSpace(
            &self,
            rect: CGRect,
            coordinateSpace: &UICoordinateSpace,
        ) -> CGRect;

        #[sel(safeAreaInsets)]
        pub fn safeAreaInsets(&self) -> UIEdgeInsets;

        #[sel(setNeedsDisplay)]
        pub fn setNeedsDisplay(&self);
    }
);

#[repr(C)]
#[derive(Debug, Clone)]
pub struct UIEdgeInsets {
    pub top: CGFloat,
    pub left: CGFloat,
    pub bottom: CGFloat,
    pub right: CGFloat,
}

unsafe impl Encode for UIEdgeInsets {
    const ENCODING: Encoding = Encoding::Struct(
        "UIEdgeInsets",
        &[
            CGFloat::ENCODING,
            CGFloat::ENCODING,
            CGFloat::ENCODING,
            CGFloat::ENCODING,
        ],
    );
}
