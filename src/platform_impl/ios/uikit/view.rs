use icrate::Foundation::{CGFloat, CGRect, NSObject};
use objc2::encode::{Encode, Encoding};
use objc2::rc::Id;
use objc2::{extern_class, extern_methods, msg_send_id, mutability, ClassType};

use super::{UICoordinateSpace, UIResponder, UIViewController};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIView;

    unsafe impl ClassType for UIView {
        #[inherits(NSObject)]
        type Super = UIResponder;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIView {
        #[method(bounds)]
        pub fn bounds(&self) -> CGRect;

        #[method(setBounds:)]
        pub fn setBounds(&self, value: CGRect);

        #[method(frame)]
        pub fn frame(&self) -> CGRect;

        #[method(setFrame:)]
        pub fn setFrame(&self, value: CGRect);

        #[method(contentScaleFactor)]
        pub fn contentScaleFactor(&self) -> CGFloat;

        #[method(setContentScaleFactor:)]
        pub fn setContentScaleFactor(&self, val: CGFloat);

        #[method(setMultipleTouchEnabled:)]
        pub fn setMultipleTouchEnabled(&self, val: bool);

        pub fn rootViewController(&self) -> Option<Id<UIViewController>> {
            unsafe { msg_send_id![self, rootViewController] }
        }

        #[method(setRootViewController:)]
        pub fn setRootViewController(&self, rootViewController: Option<&UIViewController>);

        #[method(convertRect:toCoordinateSpace:)]
        pub fn convertRect_toCoordinateSpace(
            &self,
            rect: CGRect,
            coordinateSpace: &UICoordinateSpace,
        ) -> CGRect;

        #[method(convertRect:fromCoordinateSpace:)]
        pub fn convertRect_fromCoordinateSpace(
            &self,
            rect: CGRect,
            coordinateSpace: &UICoordinateSpace,
        ) -> CGRect;

        #[method(safeAreaInsets)]
        pub fn safeAreaInsets(&self) -> UIEdgeInsets;

        #[method(setNeedsDisplay)]
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
