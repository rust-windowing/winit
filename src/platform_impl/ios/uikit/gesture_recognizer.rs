use crate::platform_impl::platform::view::WinitView;
use icrate::Foundation::{CGFloat, NSInteger, NSObject, NSUInteger};
use objc2::encode::{Encode, Encoding};
use objc2::rc::Id;
use objc2::runtime::Sel;
use objc2::{extern_class, extern_methods, msg_send_id, mutability, ClassType};

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

        #[method(initWithTarget:action:)]
        pub fn initWithTarget(&self, target: WinitView, action: Sel);
    }
);

unsafe impl Encode for UIGestureRecognizer {
    const ENCODING: Encoding = Encoding::Object;
}

// https://developer.apple.com/documentation/uikit/uigesturerecognizer/state
#[derive(Debug)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UIGestureRecognizerState {
    Possible = 0,
    Began,
    Changed,
    Ended,
    Cancelled,
    Failed,
}

unsafe impl Encode for UIGestureRecognizerState {
    const ENCODING: Encoding = NSInteger::ENCODING;
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

impl UIPinchGestureRecognizer {
    pub(crate) fn init_with_target(target: &WinitView, action: Sel) -> Id<Self> {
        unsafe { msg_send_id![Self::alloc(), initWithTarget: target, action: action] }
    }
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

impl UITapGestureRecognizer {
    pub(crate) fn init_with_target(target: &WinitView, action: Sel) -> Id<Self> {
        unsafe { msg_send_id![Self::alloc(), initWithTarget: target, action: action] }
    }
}
