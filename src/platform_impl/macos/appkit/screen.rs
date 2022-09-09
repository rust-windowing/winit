use objc2::foundation::{CGFloat, NSArray, NSDictionary, NSNumber, NSObject, NSRect, NSString};
use objc2::rc::{Id, Shared};
use objc2::runtime::Object;
use objc2::{extern_class, extern_methods, msg_send_id, ns_string, ClassType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSScreen;

    unsafe impl ClassType for NSScreen {
        type Super = NSObject;
    }
);

// TODO: Main thread marker!

extern_methods!(
    unsafe impl NSScreen {
        /// The application object must have been created.
        pub fn main() -> Option<Id<Self, Shared>> {
            unsafe { msg_send_id![Self::class(), mainScreen] }
        }

        /// The application object must have been created.
        pub fn screens() -> Id<NSArray<Self, Shared>, Shared> {
            unsafe { msg_send_id![Self::class(), screens] }
        }

        #[sel(frame)]
        pub fn frame(&self) -> NSRect;

        #[sel(visibleFrame)]
        pub fn visibleFrame(&self) -> NSRect;

        pub fn deviceDescription(
            &self,
        ) -> Id<NSDictionary<NSDeviceDescriptionKey, Object>, Shared> {
            unsafe { msg_send_id![self, deviceDescription] }
        }

        pub fn display_id(&self) -> u32 {
            let device_description = self.deviceDescription();

            // Retrieve the CGDirectDisplayID associated with this screen
            //
            // SAFETY: The value from @"NSScreenNumber" in deviceDescription is guaranteed
            // to be an NSNumber. See documentation for `deviceDescription` for details:
            // <https://developer.apple.com/documentation/appkit/nsscreen/1388360-devicedescription?language=objc>
            let obj = device_description
                .get(ns_string!("NSScreenNumber"))
                .expect("failed getting screen display id from device description");
            let obj: *const Object = obj;
            let obj: *const NSNumber = obj.cast();
            let obj: &NSNumber = unsafe { &*obj };

            obj.as_u32()
        }

        #[sel(backingScaleFactor)]
        pub fn backingScaleFactor(&self) -> CGFloat;
    }
);

pub type NSDeviceDescriptionKey = NSString;
