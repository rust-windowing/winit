//use objc2::rc::Id;
//use objc2::encode::{Encode, Encoding};
use objc2::{
    extern_class, 
    extern_methods, 
    mutability, 
    ClassType,
    runtime::{AnyObject, Sel},
    rc::{Id, Allocated},
};

use icrate::Foundation::{NSObject, CGFloat};

// https://developer.apple.com/documentation/uikit/uigesturerecognizer?language=objc
extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIGestureRecognizer;

    unsafe impl ClassType for UIGestureRecognizer {
        //#[inherits(NSObject)]
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    #[allow(non_snake_case)]
    unsafe impl UIGestureRecognizer {
        #[method(addTarget:action:)]
        fn addTarget_action(&self, target: &AnyObject, action: Sel);

        #[method(removeTarget:action:)]
        fn removeTarget_action(&self, target: &AnyObject, action: Sel);

        #[method(handleGesture:)]
        fn handleGesture(&self, gesture_recognier: &UIGestureRecognizer);

        #[method_id(initWithTarget:action:)]
        pub(crate) fn initWithTarget_action(this: Allocated<Self>, target: &AnyObject, action: Sel) -> Id<Self>;
    }
);

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIRotationGestureRecognizer;

    unsafe impl ClassType for UIRotationGestureRecognizer {
        #[inherits(NSObject)]
        type Super = UIGestureRecognizer;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIRotationGestureRecognizer {
        #[method(rotation)]
        pub(crate) fn rotation(&self) -> CGFloat;

        #[method(velocity)]
        pub(crate) fn velocity(&self) -> CGFloat;

        #[method_id(initWithTarget:action:)]
        pub(crate) fn initWithTarget_action(this: Allocated<Self>, target: &AnyObject, action: Sel) -> Id<Self>;
    }
);

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIPinchGestureRecognizer;

    unsafe impl ClassType for UIPinchGestureRecognizer {
        #[inherits(NSObject)]
        type Super = UIGestureRecognizer;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIPinchGestureRecognizer {
        #[method(rotation)]
        pub(crate) fn scale(&self) -> CGFloat;

        #[method(velocity)]
        pub(crate) fn velocity(&self) -> CGFloat;
    }
);

