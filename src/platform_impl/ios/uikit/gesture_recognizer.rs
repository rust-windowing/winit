#[allow(unused_imports)]
use super::{UIEvent, UIPress, UITouch, UIView};
use icrate::Foundation::{CGFloat, CGPoint, NSInteger, NSObject, NSTimeInterval, NSUInteger};
use objc2::{
    encode::{Encode, Encoding},
    extern_class, extern_methods, extern_protocol, mutability,
    rc::Id,
    runtime::{AnyProtocol, NSObjectProtocol, ProtocolObject, Sel},
    ClassType, ProtocolType,
};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    /// https://developer.apple.com/documentation/uikit/uigesturerecognizer
    pub(crate) struct UIGestureRecognizer;

    unsafe impl ClassType for UIGestureRecognizer {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    /// (UIGestureRecognizer)[https://developer.apple.com/documentation/uikit/uigesturerecognizerstate?language=objc]
    unsafe impl UIGestureRecognizer {
        /// (state)[https://developer.apple.com/documentation/uikit/uigesturerecognizer/1619998-state?language=objc]
        /// @property(nonatomic, readwrite) UIGestureRecognizerState state;
        #[method(state)]
        pub fn state(&self) -> UIGestureRecognizerState;

        /// (delegate)[https://developer.apple.com/documentation/uikit/uigesturerecognizer/1624207-delegate?language=objc]
        /// @property(nullable, nonatomic, weak) id<UIGestureRecognizerDelegate> delegate;
        #[method(setDelegate:)]
        pub fn set_delegate(&self, delegate: &ProtocolObject<dyn UIGestureRecognizerDelegate>);

        #[method_id(delegate)]
        pub fn get_delegate(&self) -> Id<ProtocolObject<dyn UIGestureRecognizerDelegate>>;

        /// (locationInView:)[https://developer.apple.com/documentation/uikit/uigesturerecognizer/1624219-locationinview?language=objc]
        /// - (CGPoint)locationInView:(UIView *)view;
        #[method(locationInView:)]
        pub fn location_in_view(&self, view: Option<&UIView>) -> CGPoint;

        /// (locationOfTouch:inView:)[https://developer.apple.com/documentation/uikit/uigesturerecognizer/1624201-locationoftouch?language=objc]
        /// - (CGPoint)locationOfTouch:(NSUInteger)touchIndex inView:(UIView *)view;
        #[method(locationOfTouch:inView:)]
        pub fn location_of_touch_in_view(
            &self,
            touchIndex: NSUInteger,
            view: Option<&UIView>,
        ) -> CGPoint;

        /// (numberOfTouches)[https://developer.apple.com/documentation/uikit/uigesturerecognizer/1624200-numberoftouches?language=objc]
        /// @property(nonatomic, readonly) NSUInteger numberOfTouches;
        #[method(numberOfTouches)]
        pub fn number_of_touches(&self) -> NSUInteger;

        /// (addTarget:action:)[https://developer.apple.com/documentation/uikit/uigesturerecognizer/1624230-addtarget?language=objc]
        /// - (void)addTarget:(id)target action:(SEL)action;
        #[method(addTarget:action:)]
        pub fn add_target_action(&self, target: &NSObject, action: Sel);

        /// (removeTarget:action:)[https://developer.apple.com/documentation/uikit/uigesturerecognizer/1624226-removetarget?language=objc]
        /// - (void)removeTarget:(id)target action:(SEL)action;
        #[method(removeTarget:action:)]
        pub fn remove_target_action(&self, target: &NSObject, action: Sel);

        /// (view)[https://developer.apple.com/documentation/uikit/uigesturerecognizer/1624212-view?language=objc]
        /// @property(nullable, nonatomic, readonly) UIView *view;
        #[method_id(view)]
        pub fn view(&self) -> Id<UIView>;

        /// (enabled)[https://developer.apple.com/documentation/uikit/uigesturerecognizer/1624220-enabled?language=objc]
        /// @property(nonatomic, getter=isEnabled) BOOL enabled;
        #[method(isEnabled)]
        pub fn is_enabled(&self) -> bool;
    }
);

unsafe impl Encode for UIGestureRecognizer {
    const ENCODING: Encoding = Encoding::Object;
}

extern_protocol!(
    /// (@protocol UIGestureRecognizerDelegate)[https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate?language=objc]
    pub(crate) unsafe trait UIGestureRecognizerDelegate: NSObjectProtocol {
        /// (- gestureRecognizerShouldBegin:)[https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate/1624213-gesturerecognizershouldbegin?language=objc]
        /// - (BOOL)gestureRecognizerShouldBegin:(UIGestureRecognizer *)gestureRecognizer;
        #[method(gestureRecognizerShouldBegin:)]
        fn should_begin(&self, gesture_recognizer: &UIGestureRecognizer) -> bool;

        /// (- gestureRecognizer:shouldReceiveTouch:)[https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate/1624214-gesturerecognizer?language=objc]
        /// - (BOOL)gestureRecognizer:(UIGestureRecognizer *)gestureRecognizer shouldReceiveTouch:(UITouch *)touch;
        #[method(gestureRecognizer:shouldReceiveTouch:)]
        fn should_receive_touch(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            touch: &UITouch,
        ) -> bool;

        /// (- gestureRecognizer:shouldReceivePress:)[https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate/1624216-gesturerecognizer?language=objc]
        /// - (BOOL)gestureRecognizer:(UIGestureRecognizer *)gestureRecognizer shouldReceivePress:(UIPress *)press;
        #[method(gestureRecognizer:shouldReceivePress:)]
        fn should_receive_press(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            press: &UIPress,
        ) -> bool;

        /// (- gestureRecognizer:shouldReceiveEvent:)[ https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate/3538976-gesturerecognizer?language=objc]
        /// - (BOOL)gestureRecognizer:(UIGestureRecognizer *)gestureRecognizer shouldReceiveEvent:(UIEvent *)event;
        #[method(gestureRecognizer:shouldReceiveEvent:)]
        fn should_receive_event(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            event: &UIEvent,
        ) -> bool;

        /// (- gestureRecognizer:shouldRecognizeSimultaneouslyWithGestureRecognizer:)[https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate/1624208-gesturerecognizer?language=objc]
        /// - (BOOL)gestureRecognizer:(UIGestureRecognizer *)gestureRecognizer shouldRecognizeSimultaneouslyWithGestureRecognizer:(UIGestureRecognizer *)otherGestureRecognizer;
        #[method(gestureRecognizer:shouldRecognizeSimultaneouslyWithGestureRecognizer:)]
        fn should_recognize_simultaneously(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            other_gesture_recognizer: &UIGestureRecognizer,
        ) -> bool;

        /// (- gestureRecognizer:shouldRequireFailureOfGestureRecognizer:)[https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate?language=objc]
        /// - (BOOL)gestureRecognizer:(UIGestureRecognizer *)gestureRecognizer shouldRequireFailureOfGestureRecognizer:(UIGestureRecognizer *)otherGestureRecognizer;
        #[method(gestureRecognizer:shouldRequireFailureOfGestureRecognizer:)]
        fn should_require_failure_of_gesture_recognizer(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            other_gesture_recognizer: &UIGestureRecognizer,
        ) -> bool;

        /// (- gestureRecognizer:shouldBeRequiredToFailByGestureRecognizer:)[https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate/1624222-gesturerecognizer?language=objc]
        /// - (BOOL)gestureRecognizer:(UIGestureRecognizer *)gestureRecognizer shouldBeRequiredToFailByGestureRecognizer:(UIGestureRecognizer *)otherGestureRecognizer;
        #[method(gestureRecognizer:shouldBeRequiredToFailByGestureRecognizer:)]
        fn should_be_required_to_fail_by_gesture_recognizer(
            &self,
            gesture_recognizer: &UIGestureRecognizer,
            other_gesture_recognizer: &UIGestureRecognizer,
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
