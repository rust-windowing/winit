use objc2::foundation::{MainThreadMarker, NSObject};
use objc2::rc::{Id, Shared};
use objc2::runtime::Object;
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::{NSEvent, NSResponder};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSApplication;

    unsafe impl ClassType for NSApplication {
        #[inherits(NSObject)]
        type Super = NSResponder;
    }
);

pub(crate) fn NSApp() -> Id<NSApplication, Shared> {
    let msg = "tried to access NSApp on something that was not the main thread";
    NSApplication::shared(MainThreadMarker::new().expect(msg))
}

extern_methods!(
    unsafe impl NSApplication {
        /// This can only be called on the main thread since it may initialize
        /// the application and since it's parameters may be changed by the main
        /// thread at any time (hence it is only safe to access on the main thread).
        pub fn shared(_mtm: MainThreadMarker) -> Id<Self, Shared> {
            let app: Option<_> = unsafe { msg_send_id![Self::class(), sharedApplication] };
            // SAFETY: `sharedApplication` always initializes the app if it isn't already
            unsafe { app.unwrap_unchecked() }
        }

        pub fn currentEvent(&self) -> Option<Id<NSEvent, Shared>> {
            unsafe { msg_send_id![self, currentEvent] }
        }

        // TODO: NSApplicationDelegate
        #[sel(setDelegate:)]
        pub unsafe fn setDelegate(&self, delegate: &Object);

        #[sel(hide:)]
        pub fn hide(&self, sender: Option<&Object>);

        #[sel(hideOtherApplications:)]
        pub fn hideOtherApplications(&self, sender: Option<&Object>);

        #[sel(stop:)]
        pub fn stop(&self, sender: Option<&Object>);

        #[sel(activateIgnoringOtherApps:)]
        pub fn activateIgnoringOtherApps(&self, ignore: bool);

        #[sel(run)]
        pub unsafe fn run(&self);
    }
);
