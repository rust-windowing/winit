use std::ptr;
use std::collections::RingBuf;
use super::super::ffi;
use super::ensure_thread_init;

pub struct MonitorID(pub usize);

pub fn get_available_monitors() -> RingBuf<MonitorID> {
    ensure_thread_init();
    let nb_monitors = unsafe {
        let display = ffi::XOpenDisplay(ptr::null());
        if display.is_null() {
            panic!("get_available_monitors failed");
        }
        let nb_monitors = ffi::XScreenCount(display);
        ffi::XCloseDisplay(display);
        nb_monitors
    };

    let mut monitors = RingBuf::new();
    monitors.extend(range(0, nb_monitors).map(|i| MonitorID(i as usize)));
    monitors
}

pub fn get_primary_monitor() -> MonitorID {
    ensure_thread_init();
    let primary_monitor = unsafe {
        let display = ffi::XOpenDisplay(ptr::null());
        if display.is_null() {
            panic!("get_available_monitors failed");
        }
        let primary_monitor = ffi::XDefaultScreen(display);
        ffi::XCloseDisplay(display);
        primary_monitor
    };

    MonitorID(primary_monitor as usize)
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        let MonitorID(screen_num) = *self;
        Some(format!("Monitor #{}", screen_num))
    }

    pub fn get_dimensions(&self) -> (usize, usize) {
        let dimensions = unsafe {
            let display = ffi::XOpenDisplay(ptr::null());
            let MonitorID(screen_num) = *self;
            let screen = ffi::XScreenOfDisplay(display, screen_num as i32);
            let width = ffi::XWidthOfScreen(screen);
            let height = ffi::XHeightOfScreen(screen);
            ffi::XCloseDisplay(display);
            (width as usize, height as usize)
        };

        dimensions
    }
}

