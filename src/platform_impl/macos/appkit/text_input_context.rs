use objc2::foundation::{NSObject, NSString};
use objc2::rc::{Id, Shared};
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

type NSTextInputSourceIdentifier = NSString;

extern_class!(
    /// Main-Thread-Only!
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSTextInputContext;

    unsafe impl ClassType for NSTextInputContext {
        type Super = NSObject;
    }
);

extern_methods!(
    unsafe impl NSTextInputContext {
        #[sel(invalidateCharacterCoordinates)]
        pub fn invalidateCharacterCoordinates(&self);

        #[sel(discardMarkedText)]
        pub fn discardMarkedText(&self);

        pub fn selectedKeyboardInputSource(
            &self,
        ) -> Option<Id<NSTextInputSourceIdentifier, Shared>> {
            unsafe { msg_send_id![self, selectedKeyboardInputSource] }
        }
    }
);
