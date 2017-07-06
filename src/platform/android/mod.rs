#![cfg(target_os = "android")]

extern crate android_glue;

use libc;
use std::ffi::{CString};
use std::sync::mpsc::{Receiver, channel};
use std::os::raw::c_void;
use {CreationError, WindowEvent as Event, MouseCursor};
use CreationError::OsError;
use events::ElementState::{Pressed, Released};
use events::{Touch, TouchPhase};

use std::collections::VecDeque;

use CursorState;
use WindowAttributes;
use native_monitor::NativeMonitorId;

gen_api_transition!();

pub struct Window {
    native_window: *const c_void,
    event_rx: Receiver<android_glue::Event>,
}

#[derive(Clone)]
pub struct MonitorId;

mod ffi;

#[inline]
pub fn get_available_monitors() -> VecDeque<MonitorId> {
    let mut rb = VecDeque::new();
    rb.push_back(MonitorId);
    rb
}

#[inline]
pub fn get_primary_monitor() -> MonitorId {
    MonitorId
}

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some("Primary".to_string())
    }

    #[inline]
    pub fn get_native_identifier(&self) -> NativeMonitorId {
        NativeMonitorId::Unavailable
    }

    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!()
    }
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;
#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        match self.window.event_rx.try_recv() {
            Ok(android_glue::Event::EventMotion(motion)) => {
                Some(Event::Touch(Touch {
                    phase: match motion.action {
                        android_glue::MotionAction::Down => TouchPhase::Started,
                        android_glue::MotionAction::Move => TouchPhase::Moved,
                        android_glue::MotionAction::Up => TouchPhase::Ended,
                        android_glue::MotionAction::Cancel => TouchPhase::Cancelled,
                    },
                    location: (motion.x as f64, motion.y as f64),
                    id: motion.pointer_id as u64,
                    device_id: DEVICE_ID,
                }))
            },
            Ok(android_glue::Event::InitWindow) => {
                // The activity went to foreground.
                Some(Event::Suspended(false))
            },
            Ok(android_glue::Event::TermWindow) => {
                // The activity went to background.
                Some(Event::Suspended(true))
            },
            Ok(android_glue::Event::WindowResized) |
            Ok(android_glue::Event::ConfigChanged) => {
                // Activity Orientation changed or resized.
                self.window.get_inner_size().map(|s| Event::Resized(s.0, s.1))
            },
            Ok(android_glue::Event::WindowRedrawNeeded) => {
                // The activity needs to be redrawn.
                Some(Event::Refresh)
            }
            _ => {
                None
            }
        }
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    #[inline]
    fn next(&mut self) -> Option<Event> {
        loop {
            // calling poll_events()
            if let Some(ev) = self.window.poll_events().next() {
                return Some(ev);
            }

            // TODO: Implement a proper way of sleeping on the event queue
            // timer::sleep(Duration::milliseconds(16));
        }
    }
}

impl Window {
    pub fn new(win_attribs: &WindowAttributes, _: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError>
    {
        use std::{mem, ptr};

        // not implemented
        assert!(win_attribs.min_dimensions.is_none());
        assert!(win_attribs.max_dimensions.is_none());

        let native_window = unsafe { android_glue::get_native_window() };
        if native_window.is_null() {
            return Err(OsError(format!("Android's native window is null")));
        }

        let (tx, rx) = channel();
        android_glue::add_sender(tx);
        android_glue::set_multitouch(win_attribs.multitouch);

        Ok(Window {
            native_window: native_window as *const _,
            event_rx: rx,
        })
    }

    #[inline]
    pub fn get_native_window(&self) -> *const c_void {
        self.native_window
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        false
    }

    #[inline]
    pub fn set_title(&self, _: &str) {
    }

    #[inline]
    pub fn show(&self) {
    }

    #[inline]
    pub fn hide(&self) {
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        None
    }

    #[inline]
    pub fn set_position(&self, _x: i32, _y: i32) {
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        let native_window = unsafe { android_glue::get_native_window() };

        if native_window.is_null() {
            None
        } else {
            Some((
                unsafe { ffi::ANativeWindow_getWidth(native_window as *const _) } as u32,
                unsafe { ffi::ANativeWindow_getHeight(native_window as *const _) } as u32
            ))
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.get_inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, _x: u32, _y: u32) {
    }

    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    #[inline]
    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self
        }
    }

    #[inline]
    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self
        }
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!();
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    #[inline]
    pub fn set_cursor(&self, _: MouseCursor) {
    }

    #[inline]
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        Ok(())
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    #[inline]
    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        unimplemented!();
    }
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        android_glue::wake_event_loop();
    }
}

// Constant device ID, to be removed when this backend is updated to report real device IDs.
const DEVICE_ID: ::DeviceId = ::DeviceId(DeviceId);
