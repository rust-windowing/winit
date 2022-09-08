use std::{
    self,
    ffi::c_void,
    panic::{AssertUnwindSafe, UnwindSafe},
    ptr,
    rc::Weak,
    time::Instant,
};

use crate::platform_impl::platform::{
    app_state::AppState,
    event_loop::{stop_app_on_panic, PanicInfo},
    ffi,
};
use core_foundation::base::{CFIndex, CFOptionFlags, CFRelease};
use core_foundation::date::CFAbsoluteTimeGetCurrent;
use core_foundation::runloop::{
    kCFRunLoopAfterWaiting, kCFRunLoopBeforeWaiting, kCFRunLoopCommonModes, kCFRunLoopExit,
    CFRunLoopActivity, CFRunLoopAddObserver, CFRunLoopAddTimer, CFRunLoopGetMain,
    CFRunLoopObserverCallBack, CFRunLoopObserverContext, CFRunLoopObserverCreate,
    CFRunLoopObserverRef, CFRunLoopRef, CFRunLoopTimerCreate, CFRunLoopTimerInvalidate,
    CFRunLoopTimerRef, CFRunLoopTimerSetNextFireDate,
};

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

    stop_app_on_panic(Weak::clone(&panic_info), move || {
        let _ = &panic_info;
        f(panic_info.0)
    });
}

// begin is queued with the highest priority to ensure it is processed before other observers
extern "C" fn control_flow_begin_handler(
    _: CFRunLoopObserverRef,
    activity: CFRunLoopActivity,
    panic_info: *mut c_void,
) {
    unsafe {
        control_flow_handler(panic_info, |panic_info| {
            #[allow(non_upper_case_globals)]
            match activity {
                kCFRunLoopAfterWaiting => {
                    //trace!("Triggered `CFRunLoopAfterWaiting`");
                    AppState::wakeup(panic_info);
                    //trace!("Completed `CFRunLoopAfterWaiting`");
                }
                _ => unreachable!(),
            }
        });
    }
}

// end is queued with the lowest priority to ensure it is processed after other observers
// without that, LoopDestroyed would  get sent after MainEventsCleared
extern "C" fn control_flow_end_handler(
    _: CFRunLoopObserverRef,
    activity: CFRunLoopActivity,
    panic_info: *mut c_void,
) {
    unsafe {
        control_flow_handler(panic_info, |panic_info| {
            #[allow(non_upper_case_globals)]
            match activity {
                kCFRunLoopBeforeWaiting => {
                    //trace!("Triggered `CFRunLoopBeforeWaiting`");
                    AppState::cleared(panic_info);
                    //trace!("Completed `CFRunLoopBeforeWaiting`");
                }
                kCFRunLoopExit => (), //unimplemented!(), // not expected to ever happen
                _ => unreachable!(),
            }
        });
    }
}

struct RunLoop(CFRunLoopRef);

impl RunLoop {
    unsafe fn get() -> Self {
        RunLoop(unsafe { CFRunLoopGetMain() })
    }

    unsafe fn add_observer(
        &self,
        flags: CFOptionFlags,
        priority: CFIndex,
        handler: CFRunLoopObserverCallBack,
        context: *mut CFRunLoopObserverContext,
    ) {
        let observer = unsafe {
            CFRunLoopObserverCreate(
                ptr::null_mut(),
                flags,
                ffi::TRUE, // Indicates we want this to run repeatedly
                priority,  // The lower the value, the sooner this will run
                handler,
                context,
            )
        };
        unsafe { CFRunLoopAddObserver(self.0, observer, kCFRunLoopCommonModes) };
    }
}

pub fn setup_control_flow_observers(panic_info: Weak<PanicInfo>) {
    unsafe {
        let mut context = CFRunLoopObserverContext {
            info: Weak::into_raw(panic_info) as *mut _,
            version: 0,
            retain: None,
            release: None,
            copyDescription: None,
        };
        let run_loop = RunLoop::get();
        run_loop.add_observer(
            kCFRunLoopAfterWaiting,
            CFIndex::min_value(),
            control_flow_begin_handler,
            &mut context as *mut _,
        );
        run_loop.add_observer(
            kCFRunLoopExit | kCFRunLoopBeforeWaiting,
            CFIndex::max_value(),
            control_flow_end_handler,
            &mut context as *mut _,
        );
    }
}

pub struct EventLoopWaker {
    timer: CFRunLoopTimerRef,
}

impl Drop for EventLoopWaker {
    fn drop(&mut self) {
        unsafe {
            CFRunLoopTimerInvalidate(self.timer);
            CFRelease(self.timer as _);
        }
    }
}

impl Default for EventLoopWaker {
    fn default() -> EventLoopWaker {
        extern "C" fn wakeup_main_loop(_timer: CFRunLoopTimerRef, _info: *mut c_void) {}
        unsafe {
            // Create a timer with a 0.1µs interval (1ns does not work) to mimic polling.
            // It is initially setup with a first fire time really far into the
            // future, but that gets changed to fire immediately in did_finish_launching
            let timer = CFRunLoopTimerCreate(
                ptr::null_mut(),
                std::f64::MAX,
                0.000_000_1,
                0,
                0,
                wakeup_main_loop,
                ptr::null_mut(),
            );
            CFRunLoopAddTimer(CFRunLoopGetMain(), timer, kCFRunLoopCommonModes);
            EventLoopWaker { timer }
        }
    }
}

impl EventLoopWaker {
    pub fn stop(&mut self) {
        unsafe { CFRunLoopTimerSetNextFireDate(self.timer, std::f64::MAX) }
    }

    pub fn start(&mut self) {
        unsafe { CFRunLoopTimerSetNextFireDate(self.timer, std::f64::MIN) }
    }

    pub fn start_at(&mut self, instant: Instant) {
        let now = Instant::now();
        if now >= instant {
            self.start();
        } else {
            unsafe {
                let current = CFAbsoluteTimeGetCurrent();
                let duration = instant - now;
                let fsecs =
                    duration.subsec_nanos() as f64 / 1_000_000_000.0 + duration.as_secs() as f64;
                CFRunLoopTimerSetNextFireDate(self.timer, current + fsecs)
            }
        }
    }
}
