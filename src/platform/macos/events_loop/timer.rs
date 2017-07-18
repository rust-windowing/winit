use std::mem;
use libc::c_void;
use core_foundation::base::*;
use core_foundation::runloop::*;
use core_foundation::date::*;

use super::runloop::Runloop;

// Encapsulates a CFRunLoopTimer that has a far-future time to fire, but which can be triggered
// across threads for the purpose of waking up an event loop.
pub struct Timer {
    timer: CFRunLoopTimerRef,
}

#[cfg(feature="context")]
extern "C" fn timer_callback(_timer: CFRunLoopTimerRef, _info: *mut c_void) {
    // wake the runloop, which here means "try to context switch out of the event loop coroutine"
    Runloop::wake();
}

#[cfg(not(feature="context"))]
extern "C" fn timer_callback(_timer: CFRunLoopTimerRef, _info: *mut c_void) {
    // can't really accomplish anything
}

impl Timer {
    pub fn new(interval_seconds: f64) -> Timer {
        // default to firing every year, starting one year in the future
        let interval: CFTimeInterval = interval_seconds;
        let now = unsafe { CFAbsoluteTimeGetCurrent() };
        let next_interval = now + interval;

        let mut context: CFRunLoopTimerContext = unsafe { mem::zeroed() };

        // create a timer
        let timer = unsafe {
            CFRunLoopTimerCreate(
                kCFAllocatorDefault,
                now + interval, // fireDate
                interval,       // interval
                0,              // flags
                0,              // order
                timer_callback,
                &mut context as *mut CFRunLoopTimerContext,
            )
        };

        // add it to the runloop
        unsafe {
            CFRunLoopAddTimer(CFRunLoopGetMain(), timer, kCFRunLoopCommonModes);
        }

        Timer{
            timer
        }
    }

    // Cause the timer to fire ASAP. Can be called across threads.
    pub fn trigger(&self) {
        unsafe {
            CFRunLoopTimerSetNextFireDate(self.timer, CFAbsoluteTimeGetCurrent());
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        unsafe {
            CFRunLoopRemoveTimer(CFRunLoopGetMain(), self.timer, kCFRunLoopCommonModes);
            CFRelease(self.timer as _);
        }
    }
}

// Rust doesn't know that __CFRunLoopTimer is thread safe, but the docs say it is
unsafe impl Send for Timer {}
unsafe impl Sync for Timer {}
