use objc2::foundation::{NSArray, NSObject, NSString};
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSAppearance;

    unsafe impl ClassType for NSAppearance {
        type Super = NSObject;
    }
);

type NSAppearanceName = NSString;

extern_methods!(
    unsafe impl NSAppearance {
        pub fn appearanceNamed(name: &NSAppearanceName) -> Id<Self, Shared> {
            unsafe { msg_send_id![Self::class(), appearanceNamed: name] }
        }

        pub fn bestMatchFromAppearancesWithNames(
            &self,
            appearances: &NSArray<NSAppearanceName>,
        ) -> Id<NSAppearanceName, Shared> {
            unsafe { msg_send_id![self, bestMatchFromAppearancesWithNames: appearances,] }
        }
    }
);
