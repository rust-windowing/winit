//! Various utilities for interfacing with the main run loop.
//!
//! These allow for using closures without the `Send + Sync` requirement that is otherwise required
//! when interfacing with run loops.
//!
//! See also <https://github.com/madsmtm/objc2/issues/696> for figuring this out at a lower level.

use std::cell::Cell;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_core_foundation::{
    kCFAllocatorDefault, kCFRunLoopDefaultMode, CFIndex, CFRetained, CFRunLoop, CFRunLoopActivity,
    CFRunLoopMode, CFRunLoopObserver,
};
use tracing::error;

/// Wrapper around [`CFRunLoop::main`].
#[derive(Debug)]
pub struct MainRunLoop {
    /// This is on the main thread.
    _mtm: MainThreadMarker,
    /// A cached reference to the main run loop.
    main_run_loop: CFRetained<CFRunLoop>,
}

impl MainRunLoop {
    /// Get the main run loop.
    pub fn get(_mtm: MainThreadMarker) -> Self {
        let main_run_loop = CFRunLoop::main().unwrap();
        Self { _mtm, main_run_loop }
    }

    /// Get a reference to the underlying [`CFRunLoop`].
    pub fn run_loop(&self) -> &CFRunLoop {
        &self.main_run_loop
    }

    /// Wake the main run loop.
    pub fn wake_up(&self) {
        self.main_run_loop.wake_up();
    }

    /// Submit a closure to run on the main thread as the next step in the run loop, before other
    /// event sources are processed.
    ///
    /// This is used for running event handlers, as those are not allowed to run re-entrantly.
    ///
    /// # Implementation
    ///
    /// This queuing could be implemented in the following several ways with subtle differences in
    /// timing. This list is sorted in rough order in which they are run:
    ///
    /// 1. Using `CFRunLoopPerformBlock` or `-[NSRunLoop performBlock:]`.
    ///
    /// 2. Using `-[NSObject performSelectorOnMainThread:withObject:waitUntilDone:]` or wrapping the
    ///    event in `NSEvent` and posting that to `-[NSApplication postEvent:atStart:]` (both
    ///    creates a custom `CFRunLoopSource`, and signals that to wake up the main event loop).
    ///
    ///    a. `atStart = true`.
    ///
    ///    b. `atStart = false`.
    ///
    /// 3. `dispatch_async` or `dispatch_async_f`. Note that this may appear before 2b, it does not
    ///    respect the ordering that runloop events have.
    ///
    /// We choose the first one, both for ease-of-implementation, but mostly for consistency, as we
    /// want the event to be queued in a way that preserves the order the events originally arrived
    /// in.
    ///
    /// As an example, let's assume that we receive two events from the user, a mouse click which we
    /// handled by queuing it, and a window resize which we handled immediately. If we allowed
    /// AppKit to choose the ordering when queuing the mouse event, it might get put in the back of
    /// the queue, and the events would appear out of order to the user of Winit. So we must instead
    /// put the event at the very front of the queue, to be handled as soon as possible after
    /// handling whatever event it's currently handling.
    pub fn queue_closure(&self, closure: impl FnOnce() + 'static) {
        // Convert `FnOnce()` to `Block<dyn Fn()>`.
        let closure = Cell::new(Some(closure));
        let block = block2::RcBlock::new(move || {
            debug_assert!(MainThreadMarker::new().is_some());
            if let Some(closure) = closure.take() {
                closure()
            } else {
                error!("tried to execute queued closure on main thread twice");
            }
        });

        // There are a few common modes (`kCFRunLoopCommonModes`) defined by Cocoa:
        // - `NSDefaultRunLoopMode`, alias of `kCFRunLoopDefaultMode`.
        // - `NSEventTrackingRunLoopMode`, used when mouse-dragging and live-resizing a window.
        // - `NSModalPanelRunLoopMode`, used when running a modal inside the Winit event loop.
        // - `NSConnectionReplyMode`: TODO.
        //
        // We only want to run event handlers in the default mode, as we support running a blocking
        // modal inside a Winit event handler (see [#1779]) which outrules the modal panel mode, and
        // resizing such panel window enters the event tracking run loop mode, so we can't directly
        // trigger events inside that mode either.
        //
        // Any events that are queued while running a modal or when live-resizing will instead wait,
        // and be delivered to the application afterwards.
        //
        // [#1779]: https://github.com/rust-windowing/winit/issues/1779
        let mode = unsafe { kCFRunLoopDefaultMode.unwrap() };

        let _ = self._mtm;
        // SAFETY: The runloop is valid, the mode is a `CFStringRef`, and the block is `'static`.
        //
        // Additionally, we have a `MainThreadMarker` here, which means we know we're on the main
        // thread. We also know that the run loop is the main-thread run loop, so scheduling a
        // non-`Send` block to that is allowed.
        unsafe { self.main_run_loop.perform_block(Some(mode), Some(&block)) }
    }

    /// Add an observer to the main run loop.
    pub fn add_observer(&self, observer: &MainRunLoopObserver, mode: &CFRunLoopMode) {
        // Accessing the `MainObserver`'s observer is fine here, since we're adding it to the main
        // run loop (which is on the same thread that the observer was created on).
        self.main_run_loop.add_observer(Some(&observer.observer), Some(mode));
    }

    /// Remove an observer from the main run loop.
    ///
    /// This is also done automatically when the [`MainRunLoopObserver`] is dropped.
    pub fn remove_observer(&self, observer: &MainRunLoopObserver, mode: &CFRunLoopMode) {
        // Same as in `add_observer`, accessing the main loop's observer is fine.
        self.main_run_loop.add_observer(Some(&observer.observer), Some(mode));
    }
}

/// A [`CFRunLoopObserver`] on the main thread.
///
/// This type has "ownership" semantics, and invalidates the observer when dropped.
#[derive(Debug)]
pub struct MainRunLoopObserver {
    /// This must be private, otherwise the user might add it to an arbitrary run loop (but this
    /// observer is not designed to only be on the main thread).
    observer: CFRetained<CFRunLoopObserver>,
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

        MainRunLoopObserver { observer }
    }
}

impl Drop for MainRunLoopObserver {
    fn drop(&mut self) {
        self.observer.invalidate();
    }
}
