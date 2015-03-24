use std::ptr;
use std::collections::VecDeque;
use super::super::ffi;
use super::ensure_thread_init;
use native_monitor::NativeMonitorId;

pub struct MonitorID(pub u32);

pub fn get_available_monitors() -> VecDeque<MonitorID> {
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

    let mut monitors = VecDeque::new();
    monitors.extend((0..nb_monitors).map(|i| MonitorID(i as u32)));
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

    MonitorID(primary_monitor as u32)
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        let MonitorID(screen_num) = *self;
        Some(format!("Monitor #{}", screen_num))
    }

    pub fn get_native_identifier(&self) -> NativeMonitorId {
        let MonitorID(screen_num) = *self;
        NativeMonitorId::Numeric(screen_num)
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        let dimensions = unsafe {
            let display = ffi::XOpenDisplay(ptr::null());
            let MonitorID(screen_num) = *self;
            let screen = ffi::XScreenOfDisplay(display, screen_num as i32);
            let width = ffi::XWidthOfScreen(screen);
            let height = ffi::XHeightOfScreen(screen);
            ffi::XCloseDisplay(display);
            (width as u32, height as u32)
        };

        dimensions
    }
}

