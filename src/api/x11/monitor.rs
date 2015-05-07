use std::ptr;
use std::collections::VecDeque;
use super::ffi;
use super::ensure_thread_init;
use native_monitor::NativeMonitorId;

pub struct MonitorID(pub u32);

pub fn get_available_monitors() -> VecDeque<MonitorID> {
    let xlib = ffi::Xlib::open().unwrap();        // FIXME: gracious handling

    ensure_thread_init(&xlib);
    let nb_monitors = unsafe {
        let display = (xlib.XOpenDisplay)(ptr::null());
        if display.is_null() {
            panic!("get_available_monitors failed");
        }
        let nb_monitors = (xlib.XScreenCount)(display);
        (xlib.XCloseDisplay)(display);
        nb_monitors
    };

    let mut monitors = VecDeque::new();
    monitors.extend((0..nb_monitors).map(|i| MonitorID(i as u32)));
    monitors
}

pub fn get_primary_monitor() -> MonitorID {
    let xlib = ffi::Xlib::open().unwrap();        // FIXME: gracious handling

    ensure_thread_init(&xlib);
    let primary_monitor = unsafe {
        let display = (xlib.XOpenDisplay)(ptr::null());
        if display.is_null() {
            panic!("get_available_monitors failed");
        }
        let primary_monitor = (xlib.XDefaultScreen)(display);
        (xlib.XCloseDisplay)(display);
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
        let xlib = ffi::Xlib::open().unwrap();        // FIXME: gracious handling

        let dimensions = unsafe {
            let display = (xlib.XOpenDisplay)(ptr::null());
            let MonitorID(screen_num) = *self;
            let screen = (xlib.XScreenOfDisplay)(display, screen_num as i32);
            let width = (xlib.XWidthOfScreen)(screen);
            let height = (xlib.XHeightOfScreen)(screen);
            (xlib.XCloseDisplay)(display);
            (width as u32, height as u32)
        };

        dimensions
    }
}

