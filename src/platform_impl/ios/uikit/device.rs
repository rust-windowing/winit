use icrate::Foundation::{MainThreadMarker, NSInteger, NSObject};
use objc2::encode::{Encode, Encoding};
use objc2::rc::Id;
use objc2::{extern_class, extern_methods, msg_send_id, mutability, ClassType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIDevice;

    unsafe impl ClassType for UIDevice {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIDevice {
        pub fn current(_mtm: MainThreadMarker) -> Id<Self> {
            unsafe { msg_send_id![Self::class(), currentDevice] }
        }

        #[method(userInterfaceIdiom)]
        pub fn userInterfaceIdiom(&self) -> UIUserInterfaceIdiom;
    }
);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIUserInterfaceIdiom(NSInteger);

unsafe impl Encode for UIUserInterfaceIdiom {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

impl UIUserInterfaceIdiom {
    pub const Unspecified: UIUserInterfaceIdiom = UIUserInterfaceIdiom(-1);
    pub const Phone: UIUserInterfaceIdiom = UIUserInterfaceIdiom(0);
    pub const Pad: UIUserInterfaceIdiom = UIUserInterfaceIdiom(1);
    pub const TV: UIUserInterfaceIdiom = UIUserInterfaceIdiom(2);
    pub const CarPlay: UIUserInterfaceIdiom = UIUserInterfaceIdiom(3);
}
