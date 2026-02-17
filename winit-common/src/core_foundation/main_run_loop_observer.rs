use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_core_foundation::{
    CFIndex, CFRetained, CFRunLoopActivity, CFRunLoopObserver, kCFAllocatorDefault,
};

/// A [`CFRunLoopObserver`] on the main thread.
///
/// This type has "ownership" semantics, and invalidates the observer when dropped.
#[derive(Debug)]
pub struct MainRunLoopObserver {
    /// This must be private, otherwise the user might add it to an arbitrary run loop (but this
    /// observer is not designed to only be on the main thread).
    pub(crate) observer: CFRetained<CFRunLoopObserver>,
}

impl MainRunLoopObserver {
    /// Create a new run loop observer that observes the main run loop.
    pub fn new(
        mtm: MainThreadMarker,
        activities: CFRunLoopActivity,
        repeats: bool,
        // The lower the value, the sooner this will run (inverse of a "priority").
        order: CFIndex,
        callback: impl Fn(CFRunLoopActivity) + 'static,
    ) -> Self {
        let block = RcBlock::new(move |_: *mut _, activity| {
            debug_assert!(MainThreadMarker::new().is_some());
            callback(activity)
        });

        let _ = mtm;
        // SAFETY: The callback is not Send + Sync, which would normally be unsound, but since we
        // restrict the callback to only ever be on the main thread (by taking `MainThreadMarker`,
        // and in `MainRunLoop::add_observer`), the callback doesn't have to be thread safe.
        let observer = unsafe {
            CFRunLoopObserver::with_handler(
                kCFAllocatorDefault,
                activities.0,
                repeats,
                order,
                Some(&block),
            )
        }
        .unwrap();

        Self { observer }
    }
}

impl Drop for MainRunLoopObserver {
    fn drop(&mut self) {
        self.observer.invalidate();
    }
}
