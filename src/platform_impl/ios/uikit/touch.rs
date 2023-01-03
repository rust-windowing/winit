use objc2::encode::{Encode, Encoding};
use objc2::foundation::{CGFloat, CGPoint, NSInteger, NSObject};
use objc2::{extern_class, extern_methods, ClassType};

use super::UIView;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UITouch;

    unsafe impl ClassType for UITouch {
        type Super = NSObject;
    }
);

extern_methods!(
    unsafe impl UITouch {
        #[sel(locationInView:)]
        pub fn locationInView(&self, view: Option<&UIView>) -> CGPoint;

        #[sel(type)]
        pub fn type_(&self) -> UITouchType;

        #[sel(force)]
        pub fn force(&self) -> CGFloat;

        #[sel(maximumPossibleForce)]
        pub fn maximumPossibleForce(&self) -> CGFloat;

        #[sel(altitudeAngle)]
        pub fn altitudeAngle(&self) -> CGFloat;

        #[sel(phase)]
        pub fn phase(&self) -> UITouchPhase;
    }
);

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UITouchType {
    Direct = 0,
    Indirect,
    Pencil,
}

unsafe impl Encode for UITouchType {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

#[derive(Debug)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UITouchPhase {
    Began = 0,
    Moved,
    Stationary,
    Ended,
    Cancelled,
}

unsafe impl Encode for UITouchPhase {
    const ENCODING: Encoding = NSInteger::ENCODING;
}
