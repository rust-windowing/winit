use icrate::Foundation::{CGFloat, NSArray, NSInteger, NSObject, NSTimeInterval};
use objc2::{
    encode::{Encode, Encoding},
    extern_class, extern_methods, mutability,
    rc::Id,
    ClassType,
};

use super::{UIGestureRecognizer, UIKey, UIResponder, UIWindow};

extern_class!(
    /// https://developer.apple.com/documentation/uikit/uipress?language=objc
    /// @interface UIPress : NSObject
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIPress;

    unsafe impl ClassType for UIPress {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIPress {
        /// https://developer.apple.com/documentation/uikit/uipress/1620364-force?language=objc
        /// @property(nonatomic, readonly) CGFloat force;
        #[method(force)]
        pub fn force(&self) -> CGFloat;

        /// https://developer.apple.com/documentation/uikit/uipress/1620376-gesturerecognizers?language=objc
        /// @property(nullable, nonatomic, readonly, copy) NSArray<UIGestureRecognizer *> *gestureRecognizers;
        #[method_id(gestureRecognizers)]
        pub fn gesture_recognizers(&self) -> Option<Id<NSArray<UIGestureRecognizer>>>;

        /// https://developer.apple.com/documentation/uikit/uipress/1620374-responder?language=objc
        /// @property(nullable, nonatomic, readonly, strong) UIResponder *responder;
        #[method_id(responder)]
        pub fn responder(&self) -> Option<Id<UIResponder>>;

        /// https://developer.apple.com/documentation/uikit/uipress/1620366-window?language=objc
        /// @property(nullable, nonatomic, readonly, strong) UIWindow *window;
        #[method_id(window)]
        pub fn window(&self) -> Option<Id<UIWindow>>;

        /// https://developer.apple.com/documentation/uikit/uipress/3526315-key?language=objc
        /// @property(nonatomic, nullable, readonly) UIKey *key;
        #[method_id(key)]
        pub fn key(&self) -> Option<Id<UIKey>>;

        /// https://developer.apple.com/documentation/uikit/uipress/1620370-type?language=objc
        /// @property(nonatomic, readonly) UIPressType type;
        #[method(type)]
        pub fn type_(&self) -> UIPressType;

        /// https://developer.apple.com/documentation/uikit/uipress/1620367-phase?language=objc
        /// @property(nonatomic, readonly) UIPressPhase phase;
        #[method(phase)]
        pub fn phase(&self) -> UIPressPhase;

        /// https://developer.apple.com/documentation/uikit/uipress/1620360-timestamp?language=objc
        /// @property(nonatomic, readonly) NSTimeInterval timestamp;
        #[method(timestamp)]
        pub fn timestamp(&self) -> NSTimeInterval;
    }
);

/// https://developer.apple.com/documentation/uikit/uipresstype?language=objc
/// typedef enum UIPressType : NSInteger {
///    ...
/// } UIPressType;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIPressType(NSInteger);

unsafe impl Encode for UIPressType {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

#[allow(dead_code)]
impl UIPressType {
    pub const UpArrow: Self = Self(0);
    pub const DownArrow: Self = Self(1);
    pub const LeftArrow: Self = Self(2);
    pub const RightArrow: Self = Self(3);

    pub const Select: Self = Self(4);
    pub const Menu: Self = Self(5);
    pub const PlayPause: Self = Self(6);

    pub const PageUp: Self = Self(30);
    pub const PageDown: Self = Self(31);
}

/// https://developer.apple.com/documentation/uikit/uipressphase?language=objc
/// typedef enum UIPressPhase : NSInteger {
///    ...
/// } UIPressPhase;
pub struct UIPressPhase(NSInteger);

unsafe impl Encode for UIPressPhase {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

#[allow(dead_code)]
impl UIPressPhase {
    /// whenever a button press begins.
    pub const PhaseBegan: Self = Self(0);
    /// whenever a button moves.
    pub const PhaseChanged: Self = Self(0);
    /// whenever a buttons was pressed and is still being held down.
    pub const PhaseStationary: Self = Self(0);
    /// whenever a button is released.
    pub const PhaseEnded: Self = Self(0);
    /// whenever a button press doesn't end but we need to stop tracking.
    pub const PhaseCancelled: Self = Self(0);
}
