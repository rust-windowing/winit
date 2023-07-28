use icrate::Foundation::{CGFloat, CGRect, MainThreadMarker, NSArray, NSInteger, NSObject};
use objc2::encode::{Encode, Encoding};
use objc2::rc::Id;
use objc2::{extern_class, extern_methods, msg_send_id, mutability, ClassType};

use super::{UICoordinateSpace, UIScreenMode};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIScreen;

    unsafe impl ClassType for UIScreen {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIScreen {
        pub fn main(_mtm: MainThreadMarker) -> Id<Self> {
            unsafe { msg_send_id![Self::class(), mainScreen] }
        }

        pub fn screens(_mtm: MainThreadMarker) -> Id<NSArray<Self>> {
            unsafe { msg_send_id![Self::class(), screens] }
        }

        #[method(bounds)]
        pub fn bounds(&self) -> CGRect;

        #[method(scale)]
        pub fn scale(&self) -> CGFloat;

        #[method(nativeBounds)]
        pub fn nativeBounds(&self) -> CGRect;

        #[method(nativeScale)]
        pub fn nativeScale(&self) -> CGFloat;

        #[method(maximumFramesPerSecond)]
        pub fn maximumFramesPerSecond(&self) -> NSInteger;

        pub fn mirroredScreen(&self) -> Id<Self> {
            unsafe { msg_send_id![Self::class(), mirroredScreen] }
        }

        pub fn preferredMode(&self) -> Option<Id<UIScreenMode>> {
            unsafe { msg_send_id![self, preferredMode] }
        }

        #[method(setCurrentMode:)]
        pub fn setCurrentMode(&self, mode: Option<&UIScreenMode>);

        pub fn availableModes(&self) -> Id<NSArray<UIScreenMode>> {
            unsafe { msg_send_id![self, availableModes] }
        }

        #[method(setOverscanCompensation:)]
        pub fn setOverscanCompensation(&self, overscanCompensation: UIScreenOverscanCompensation);

        pub fn coordinateSpace(&self) -> Id<UICoordinateSpace> {
            unsafe { msg_send_id![self, coordinateSpace] }
        }
    }
);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIScreenOverscanCompensation(NSInteger);

unsafe impl Encode for UIScreenOverscanCompensation {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

#[allow(dead_code)]
impl UIScreenOverscanCompensation {
    pub const Scale: Self = Self(0);
    pub const InsetBounds: Self = Self(1);
    pub const None: Self = Self(2);
}
