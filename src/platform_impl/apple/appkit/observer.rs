//! Utilities for working with `CFRunLoop`.
//!
//! See Apple's documentation on Run Loops for details:
//! <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/RunLoopManagement/RunLoopManagement.html>
use std::cell::Cell;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, UnwindSafe};
use std::ptr;
use std::rc::Weak;
use std::time::Instant;

use objc2::MainThreadMarker;
use objc2_core_foundation::{
    kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFAbsoluteTimeGetCurrent, CFIndex, CFRetained,
    CFRunLoop, CFRunLoopActivity, CFRunLoopAddObserver, CFRunLoopAddTimer, CFRunLoopGetMain,
    CFRunLoopObserver, CFRunLoopObserverCallBack, CFRunLoopObserverContext,
    CFRunLoopObserverCreate, CFRunLoopPerformBlock, CFRunLoopTimer, CFRunLoopTimerCreate,
    CFRunLoopTimerInvalidate, CFRunLoopTimerSetNextFireDate, CFRunLoopWakeUp,
};
use tracing::error;

use super::app_state::AppState;
use super::event_loop::{stop_app_on_panic, PanicInfo};

unsafe fn control_flow_handler<F>(panic_info: *mut c_void, f: F)
where
    F: FnOnce(Weak<PanicInfo>) + UnwindSafe,
{
    let info_from_raw = unsafe { Weak::from_raw(panic_info as *mut PanicInfo) };
    // Asserting unwind safety on this type should be fine because `PanicInfo` is
    // `RefUnwindSafe` and `Rc<T>` is `UnwindSafe` if `T` is `RefUnwindSafe`.
    let panic_info = AssertUnwindSafe(Weak::clone(&info_from_raw));
    // `from_raw` takes ownership of the data behind the pointer.
    // But if this scope takes ownership of the weak pointer, then
    // the weak pointer will get free'd at the end of the scope.
    // However we want to keep that weak reference around after the function.
    std::mem::forget(info_from_raw);

    let mtm = MainThreadMarker::new().unwrap();
    stop_app_on_panic(mtm, Weak::clone(&panic_info), move || {
        let _ = &panic_info;
        f(panic_info.0)
    });
}

// begin is queued with the highest priority to ensure it is processed before other observers
extern "C-unwind" fn control_flow_begin_handler(
    _: *mut CFRunLoopObserver,
    activity: CFRunLoopActivity,
    panic_info: *mut c_void,
) {
    unsafe {
        control_flow_handler(panic_info, |panic_info| {
            #[allow(non_upper_case_globals)]
            match activity {
                CFRunLoopActivity::AfterWaiting => {
                    // trace!("Triggered `CFRunLoopAfterWaiting`");
                    AppState::get(MainThreadMarker::new().unwrap()).wakeup(panic_info);
                    // trace!("Completed `CFRunLoopAfterWaiting`");
                },
                _ => unreachable!(),
            }
        });
    }
}

// end is queued with the lowest priority to ensure it is processed after other observers
// without that, LoopExiting would  get sent after AboutToWait
extern "C-unwind" fn control_flow_end_handler(
    _: *mut CFRunLoopObserver,
    activity: CFRunLoopActivity,
    panic_info: *mut c_void,
) {
    unsafe {
        control_flow_handler(panic_info, |panic_info| {
            #[allow(non_upper_case_globals)]
            match activity {
                CFRunLoopActivity::BeforeWaiting => {
                    // trace!("Triggered `CFRunLoopBeforeWaiting`");
                    AppState::get(MainThreadMarker::new().unwrap()).cleared(panic_info);
                    // trace!("Completed `CFRunLoopBeforeWaiting`");
                },
                CFRunLoopActivity::Exit => (), /* unimplemented!(), // not expected to ever happen */
                _ => unreachable!(),
            }
        });
    }
}

#[derive(Debug)]
pub struct RunLoop(CFRetained<CFRunLoop>);

impl RunLoop {
    pub fn main(mtm: MainThreadMarker) -> Self {
        // SAFETY: We have a MainThreadMarker here, which means we know we're on the main thread, so
        // scheduling (and scheduling a non-`Send` block) to that thread is allowed.
        let _ = mtm;
        RunLoop(unsafe { CFRunLoopGetMain() }.unwrap())
    }

    pub fn wakeup(&self) {
        unsafe { CFRunLoopWakeUp(&self.0) }
    }

    unsafe fn add_observer(
        &self,
        flags: CFRunLoopActivity,
        // The lower the value, the sooner this will run
        priority: CFIndex,
        handler: CFRunLoopObserverCallBack,
        context: *mut CFRunLoopObserverContext,
    ) {
        let observer =
            unsafe { CFRunLoopObserverCreate(None, flags.0, true, priority, handler, context) }
                .unwrap();
        unsafe { CFRunLoopAddObserver(&self.0, Some(&observer), kCFRunLoopCommonModes) };
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

        // SAFETY: The runloop is valid, the mode is a `CFStringRef`, and the block is `'static`.
        unsafe { CFRunLoopPerformBlock(&self.0, Some(mode), Some(&block)) }
    }
}

pub fn setup_control_flow_observers(mtm: MainThreadMarker, panic_info: Weak<PanicInfo>) {
    let run_loop = RunLoop::main(mtm);
    unsafe {
        let mut context = CFRunLoopObserverContext {
            info: Weak::into_raw(panic_info) as *mut _,
            version: 0,
            retain: None,
            release: None,
            copyDescription: None,
        };
        run_loop.add_observer(
            CFRunLoopActivity::AfterWaiting,
            CFIndex::MIN,
            Some(control_flow_begin_handler),
            &mut context as *mut _,
        );
        run_loop.add_observer(
            CFRunLoopActivity::Exit | CFRunLoopActivity::BeforeWaiting,
            CFIndex::MAX,
            Some(control_flow_end_handler),
            &mut context as *mut _,
        );
    }
}

#[derive(Debug)]
pub struct EventLoopWaker {
    timer: CFRetained<CFRunLoopTimer>,

    /// An arbitrary instant in the past, that will trigger an immediate wake
    /// We save this as the `next_fire_date` for consistency so we can
    /// easily check if the next_fire_date needs updating.
    start_instant: Instant,

    /// This is what the `NextFireDate` has been set to.
    /// `None` corresponds to `waker.stop()` and `start_instant` is used
    /// for `waker.start()`
    next_fire_date: Option<Instant>,
}

impl Drop for EventLoopWaker {
    fn drop(&mut self) {
        unsafe { CFRunLoopTimerInvalidate(&self.timer) };
    }
}

impl EventLoopWaker {
    pub(crate) fn new() -> Self {
        extern "C-unwind" fn wakeup_main_loop(_timer: *mut CFRunLoopTimer, _info: *mut c_void) {}
        unsafe {
            // Create a timer with a 0.1µs interval (1ns does not work) to mimic polling.
            // It is initially setup with a first fire time really far into the
            // future, but that gets changed to fire immediately in did_finish_launching
            let timer = CFRunLoopTimerCreate(
                None,
                f64::MAX,
                0.000_000_1,
                0,
                0,
                Some(wakeup_main_loop),
                ptr::null_mut(),
            )
            .unwrap();
            CFRunLoopAddTimer(&CFRunLoopGetMain().unwrap(), Some(&timer), kCFRunLoopCommonModes);
            Self { timer, start_instant: Instant::now(), next_fire_date: None }
        }
    }

    pub fn stop(&mut self) {
        if self.next_fire_date.is_some() {
            self.next_fire_date = None;
            unsafe { CFRunLoopTimerSetNextFireDate(&self.timer, f64::MAX) };
        }
    }

    pub fn start(&mut self) {
        if self.next_fire_date != Some(self.start_instant) {
            self.next_fire_date = Some(self.start_instant);
            unsafe { CFRunLoopTimerSetNextFireDate(&self.timer, f64::MIN) };
        }
    }

    pub fn start_at(&mut self, instant: Option<Instant>) {
        let now = Instant::now();
        match instant {
            Some(instant) if now >= instant => {
                self.start();
            },
            Some(instant) => {
                if self.next_fire_date != Some(instant) {
                    self.next_fire_date = Some(instant);
                    unsafe {
                        let current = CFAbsoluteTimeGetCurrent();
                        let duration = instant - now;
                        let fsecs = duration.subsec_nanos() as f64 / 1_000_000_000.0
                            + duration.as_secs() as f64;
                        CFRunLoopTimerSetNextFireDate(&self.timer, current + fsecs);
                    }
                }
            },
            None => {
                self.stop();
            },
        }
    }
}
