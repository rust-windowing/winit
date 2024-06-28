use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSNotificationName, NSObject};

/// Observe the given notification.
///
/// This is used in Winit as an alternative to declaring an application delegate, as we want to
/// give the user full control over those.
pub fn create_observer(
    center: &NSNotificationCenter,
    name: &NSNotificationName,
    handler: impl Fn(&NSNotification) + 'static,
) -> Retained<NSObject> {
    let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        handler(unsafe { notification.as_ref() });
    });
    unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(name),
            None, // No sender filter
            None, // No queue, run on posting thread (i.e. main thread)
            &block,
        )
    }
}
