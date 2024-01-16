use icrate::Foundation::{CGFloat, CGPoint, NSInteger, NSObject, NSTimeInterval};
use objc2::encode::{Encode, Encoding};
use objc2::{extern_class, extern_methods, mutability, ClassType};

use super::{UIKey, UIResponder, UIWindow};

#[cfg(feature = "i_dont_know_how_to_make_this_work")]
use super::UIGestureRecognizer;
use super::UIView;
#[cfg(feature = "i_dont_know_how_to_make_this_work")]
use objc2::rc::Id;

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIPress;

    unsafe impl ClassType for UIPress {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIPress {
        #[method(locationInView:)]
        pub fn locationInView(&self, view: Option<&UIView>) -> CGPoint;

        #[method(key)]
        pub fn key(&self) -> Option<&UIKey>;

        #[method(type)]
        pub fn type_(&self) -> UIPressType;

        #[cfg(feature = "i_dont_know_how_to_make_this_work")]
        #[method_id(gestureRecognizers)]
        pub fn gesture_recognizers(&self) -> Id<NSArray<&UIGestureRecognizer>>;

        #[method(responder)]
        pub fn responder(&self) -> &UIResponder;

        #[method(window)]
        pub fn window(&self) -> &UIWindow;

        #[method(force)]
        pub fn force(&self) -> CGFloat;

        #[method(maximumPossibleForce)]
        pub fn maximumPossibleForce(&self) -> CGFloat;

        #[method(altitudeAngle)]
        pub fn altitudeAngle(&self) -> CGFloat;

        #[method(phase)]
        pub fn phase(&self) -> UIPressPhase;

        #[method(timestamp)]
        pub fn timestamp(&self) -> NSTimeInterval;
    }
);

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UIPressType {
    TypeUpArrow = 0,
    TypeDownArrow,
    TypeLeftArrow,
    TypeRightArrow,

    TypeSelect,
    TypeMenu,
    TypePlayPause,

    TypePageUp = 30,
    TypePageDown = 31,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UIPressPhase {
    PhaseBegan,      // whenever a button press begins.
    PhaseChanged,    // whenever a button moves.
    PhaseStationary, // whenever a buttons was pressed and is still being held down.
    PhaseEnded,      // whenever a button is released.
    PhaseCancelled,  // whenever a button press doesn't end but we need to stop tracking.
}

unsafe impl Encode for UIPressType {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

unsafe impl Encode for UIPressPhase {
    const ENCODING: Encoding = NSInteger::ENCODING;
}
