#[allow(unused_imports)]
use super::{UIEvent, UIPress, UITouch, UIView};
use icrate::Foundation::{CGFloat, CGPoint, NSInteger, NSObject, NSTimeInterval, NSUInteger};
use objc2::{
    encode::{Encode, Encoding},
    extern_class, extern_methods, extern_protocol, mutability,
    runtime::{AnyProtocol, NSObjectProtocol, ProtocolObject, Sel},
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
        // https://developer.apple.com/documentation/uikit/uigesturerecognizerstate?language=objc
        #[method(state)]
        pub fn state(&self) -> UIGestureRecognizerState;

        #[method(setDelegate:)]
        pub fn set_delegate(&self, delegate: &ProtocolObject<dyn UIGestureRecognizerDelegate>);

        #[method(delegate)]
        pub fn get_delegate(&self) -> &ProtocolObject<dyn UIGestureRecognizerDelegate>;

        #[method(locationInView:)]
        pub fn location_in_view(&self, view: Option<&UIView>) -> CGPoint;

        #[method(locationOfTouch:inView:)]
        pub fn location_of_touch_in_view(
            &self,
            touchIndex: NSUInteger,
            view: Option<&UIView>,
        ) -> CGPoint;

        #[method(numberOfTouches)]
        pub fn number_of_touches(&self) -> NSUInteger;

        #[method(addTarget:action:)]
        pub fn add_target_action(&self, target: &NSObject, action: Sel);

        #[method(removeTarget:action:)]
        pub fn remove_target_action(&self, target: &NSObject, action: Sel);

        #[method(view)]
        pub fn view(&self) -> &UIView;

        #[method(isEnabled)]
        pub fn is_enabled(&self) -> bool;
    }
);

unsafe impl Encode for UIGestureRecognizer {
    const ENCODING: Encoding = Encoding::Object;
}

// (UIGestureRecognizerDelegate )[https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate?language=objc]
extern_protocol!(
    pub(crate) unsafe trait UIGestureRecognizerDelegate: NSObjectProtocol {
        #[method(gestureRecognizer:shouldRecognizeSimultaneouslyWithGestureRecognizer:)]
        fn should_recognize_simultaneously(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            other_gesture_recognizer: &UIGestureRecognizer,
        ) -> bool;

        #[method(gestureRecognizer:shouldRequireFailureOfGestureRecognizer:)]
        fn should_require_failure_of_gesture_recognizer(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            other_gesture_recognizer: &UIGestureRecognizer,
        ) -> bool;

        #[method(gestureRecognizer:shouldBeRequiredToFailByGestureRecognizer:)]
        fn should_be_required_to_fail_by_gesture_recognizer(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            other_gesture_recognizer: &UIGestureRecognizer,
        ) -> bool;

        #[method(gestureRecognizerShouldBegin:)]
        fn should_begin(&self, gesture_recognizer: &UIGestureRecognizer) -> bool;

        #[method(gestureRecognizer:shouldReceiveTouch:)]
        fn should_receive_touch(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            touch: &UITouch,
        ) -> bool;

        #[method(gestureRecognizer:shouldReceivePress:)]
        fn should_receive_press(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            press: &UIPress,
        ) -> bool;

        #[method(gestureRecognizer:shouldReceiveEvent:)]
        fn should_receive_event(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            event: &UIEvent,
        ) -> bool;
    }

    unsafe impl ProtocolType for dyn UIGestureRecognizerDelegate {
        const NAME: &'static str = "UIGestureRecognizerDelegate";
    }
);

pub fn register_protocol() {
    log::debug!("Registering protocol UIGestureRecognizerDelegate");
    let _: Option<&AnyProtocol> = <dyn UIGestureRecognizerDelegate>::protocol();
}

unsafe impl Encode for dyn UIGestureRecognizerDelegate {
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
        pub fn translation_in_view(&self, view: &UIView) -> CGPoint;

        #[method(setTranslation:inView:)]
        pub fn set_translation_in_view(&self, translation: CGPoint, view: &UIView);

        #[method(velocityInView:)]
        pub fn velocity_in_view(&self, view: &UIView) -> CGPoint;
    }
);

unsafe impl Encode for UIPanGestureRecognizer {
    const ENCODING: Encoding = Encoding::Object;
}

// https://developer.apple.com/documentation/uikit/uilongpressgesturerecognizer?language=objc
extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UILongPressGestureRecognizer;

    unsafe impl ClassType for UILongPressGestureRecognizer {
        type Super = UIGestureRecognizer;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UILongPressGestureRecognizer {
        #[method(setMinimumPressDuration:)]
        pub fn setMinimumPressDuration(&self, duration: NSTimeInterval);

        #[method(setNumberOfTouchesRequired:)]
        pub fn setNumberOfTouchesRequired(&self, number_of_touches_required: NSUInteger);

        #[method(setNumberOfTapsRequired:)]
        pub fn setNumberOfTapsRequired(&self, number_of_taps_required: NSUInteger);

        #[method(setAllowableMovement:)]
        pub fn setAllowableMovement(&self, allowable_movement: CGFloat);
    }
);

unsafe impl Encode for UILongPressGestureRecognizer {
    const ENCODING: Encoding = Encoding::Object;
}
