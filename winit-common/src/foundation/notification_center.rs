use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSNotificationName, NSObjectProtocol,
};

/// Observe the given notification.
///
/// This is used in Winit as an alternative to declaring an application delegate, as we want to
/// give the user full control over those.
///
/// The handler will be run on the current thread.
pub fn create_observer(
    center: &NSNotificationCenter,
    name: &NSNotificationName,
    handler: impl Fn(&NSNotification) + 'static,
) -> Retained<ProtocolObject<dyn NSObjectProtocol>> {
    #[cfg(debug_assertions)]
    let thread_id = std::thread::current().id();
    let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        #[cfg(debug_assertions)]
        assert_eq!(thread_id, std::thread::current().id(), "must run on posting thread");
        handler(unsafe { notification.as_ref() });
    });
    unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(name),
            None, // No sender filter
            None, // No queue, run on posting thread
            &block,
        )
    }
}
