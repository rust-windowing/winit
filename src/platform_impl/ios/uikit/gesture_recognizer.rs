use super::UIView;
use icrate::Foundation::{CGFloat, CGPoint, NSInteger, NSObject, NSUInteger};
use objc2::{
    encode::{Encode, Encoding},
    extern_class, extern_methods, extern_protocol, mutability,
    rc::Id,
    runtime::{NSObjectProtocol, ProtocolObject},
    ClassType, ProtocolType,
};

// https://developer.apple.com/documentation/uikit/uigesturerecognizer
extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIGestureRecognizer;

    unsafe impl ClassType for UIGestureRecognizer {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIGestureRecognizer {
        #[method(state)]
        pub fn state(&self) -> UIGestureRecognizerState;

        /// (delegate)[https://developer.apple.com/documentation/uikit/uigesturerecognizer/1624207-delegate?language=objc]
        /// @property(nullable, nonatomic, weak) id<UIGestureRecognizerDelegate> delegate;
        #[method(setDelegate:)]
        pub fn setDelegate(&self, delegate: &ProtocolObject<dyn UIGestureRecognizerDelegate>);

        #[method_id(delegate)]
        pub fn delegate(&self) -> Id<ProtocolObject<dyn UIGestureRecognizerDelegate>>;
    }
);

unsafe impl Encode for UIGestureRecognizer {
    const ENCODING: Encoding = Encoding::Object;
}

// https://developer.apple.com/documentation/uikit/uigesturerecognizer/state
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIGestureRecognizerState(NSInteger);

unsafe impl Encode for UIGestureRecognizerState {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

#[allow(dead_code)]
impl UIGestureRecognizerState {
    pub const Possible: Self = Self(0);
    pub const Began: Self = Self(1);
    pub const Changed: Self = Self(2);
    pub const Ended: Self = Self(3);
    pub const Cancelled: Self = Self(4);
    pub const Failed: Self = Self(5);
}

// https://developer.apple.com/documentation/uikit/uipinchgesturerecognizer
extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIPinchGestureRecognizer;

    unsafe impl ClassType for UIPinchGestureRecognizer {
        type Super = UIGestureRecognizer;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIPinchGestureRecognizer {
        #[method(scale)]
        pub fn scale(&self) -> CGFloat;

        #[method(velocity)]
        pub fn velocity(&self) -> CGFloat;
    }
);

unsafe impl Encode for UIPinchGestureRecognizer {
    const ENCODING: Encoding = Encoding::Object;
}

// https://developer.apple.com/documentation/uikit/uirotationgesturerecognizer
extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIRotationGestureRecognizer;

    unsafe impl ClassType for UIRotationGestureRecognizer {
        type Super = UIGestureRecognizer;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIRotationGestureRecognizer {
        #[method(rotation)]
        pub fn rotation(&self) -> CGFloat;

        #[method(velocity)]
        pub fn velocity(&self) -> CGFloat;
    }
);

unsafe impl Encode for UIRotationGestureRecognizer {
    const ENCODING: Encoding = Encoding::Object;
}

// https://developer.apple.com/documentation/uikit/uitapgesturerecognizer
extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UITapGestureRecognizer;

    unsafe impl ClassType for UITapGestureRecognizer {
        type Super = UIGestureRecognizer;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UITapGestureRecognizer {
        #[method(setNumberOfTapsRequired:)]
        pub fn setNumberOfTapsRequired(&self, number_of_taps_required: NSUInteger);

        #[method(setNumberOfTouchesRequired:)]
        pub fn setNumberOfTouchesRequired(&self, number_of_touches_required: NSUInteger);
    }
);

unsafe impl Encode for UITapGestureRecognizer {
    const ENCODING: Encoding = Encoding::Object;
}

// https://developer.apple.com/documentation/uikit/uipangesturerecognizer
extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIPanGestureRecognizer;

    unsafe impl ClassType for UIPanGestureRecognizer {
        type Super = UIGestureRecognizer;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIPanGestureRecognizer {
        #[method(translationInView:)]
        pub fn translationInView(&self, view: &UIView) -> CGPoint;

        #[method(setTranslation:inView:)]
        pub fn setTranslationInView(&self, translation: CGPoint, view: &UIView);

        #[method(velocityInView:)]
        pub fn velocityInView(&self, view: &UIView) -> CGPoint;

        #[method(setMinimumNumberOfTouches:)]
        pub fn setMinimumNumberOfTouches(&self, minimum_number_of_touches: NSUInteger);

        #[method(minimumNumberOfTouches)]
        pub fn minimumNumberOfTouches(&self) -> NSUInteger;

        #[method(setMaximumNumberOfTouches:)]
        pub fn setMaximumNumberOfTouches(&self, maximum_number_of_touches: NSUInteger);

        #[method(maximumNumberOfTouches)]
        pub fn maximumNumberOfTouches(&self) -> NSUInteger;
    }
);

extern_protocol!(
    /// (@protocol UIGestureRecognizerDelegate)[https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate?language=objc]
    pub(crate) unsafe trait UIGestureRecognizerDelegate: NSObjectProtocol {}

    unsafe impl ProtocolType for dyn UIGestureRecognizerDelegate {
        const NAME: &'static str = "UIGestureRecognizerDelegate";
    }
);
