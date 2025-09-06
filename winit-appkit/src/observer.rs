use std::ffi::c_void;
use std::ptr;
use std::time::Instant;

use objc2_core_foundation::{
    kCFRunLoopCommonModes, CFAbsoluteTimeGetCurrent, CFRetained, CFRunLoop, CFRunLoopTimer,
};

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
        self.timer.invalidate();
    }
}

impl EventLoopWaker {
    pub(crate) fn new() -> Self {
        extern "C-unwind" fn wakeup_main_loop(_timer: *mut CFRunLoopTimer, _info: *mut c_void) {}
        unsafe {
            // Create a timer with a 0.1µs interval (1ns does not work) to mimic polling.
            // It is initially setup with a first fire time really far into the
            // future, but that gets changed to fire immediately in did_finish_launching
            let timer = CFRunLoopTimer::new(
                None,
                f64::MAX,
                0.000_000_1,
                0,
                0,
                Some(wakeup_main_loop),
                ptr::null_mut(),
            )
            .unwrap();
            CFRunLoop::main().unwrap().add_timer(Some(&timer), kCFRunLoopCommonModes);
            Self { timer, start_instant: Instant::now(), next_fire_date: None }
        }
    }

    pub fn stop(&mut self) {
        if self.next_fire_date.is_some() {
            self.next_fire_date = None;
            self.timer.set_next_fire_date(f64::MAX);
        }
    }

    pub fn start(&mut self) {
        if self.next_fire_date != Some(self.start_instant) {
            self.next_fire_date = Some(self.start_instant);
            self.timer.set_next_fire_date(f64::MIN);
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
                    let current = CFAbsoluteTimeGetCurrent();
                    let duration = instant - now;
                    let fsecs = duration.subsec_nanos() as f64 / 1_000_000_000.0
                        + duration.as_secs() as f64;
                    self.timer.set_next_fire_date(current + fsecs);
                }
            },
            None => {
                self.stop();
            },
        }
    }
}
