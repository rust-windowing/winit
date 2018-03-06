#![cfg(target_os = "android")]

extern crate android_glue;

use libc;
use std::sync::mpsc::{Receiver, channel};
use std::os::raw::c_void;
use {CreationError, Event, WindowEvent, MouseCursor};
use CreationError::OsError;
use WindowId as RootWindowId;
use events::{Touch, TouchPhase};
use window::MonitorId as RootMonitorId;

use std::collections::VecDeque;
use std::cell::RefCell;

use CursorState;
use WindowAttributes;

pub struct EventsLoop {
    event_rx: Receiver<android_glue::Event>,
    suspend_callback: RefCell<Option<Box<Fn(bool) -> ()>>>
}

#[derive(Clone)]
pub struct EventsLoopProxy;

impl EventsLoop {
    pub fn new() -> EventsLoop {
        let (tx, rx) = channel();
        android_glue::add_sender(tx);
        EventsLoop {
            event_rx: rx,
            suspend_callback: RefCell::new(None),
        }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut rb = VecDeque::new();
        rb.push_back(MonitorId);
        rb
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        MonitorId
    }


    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(::Event)
    {
        while let Ok(event) = self.event_rx.try_recv() {

            let e = match event{
                android_glue::Event::EventMotion(motion) => {
                    Some(Event::WindowEvent {
                        window_id: RootWindowId(WindowId),
                        event: WindowEvent::Touch(Touch {
                            phase: match motion.action {
                                android_glue::MotionAction::Down => TouchPhase::Started,
                                android_glue::MotionAction::Move => TouchPhase::Moved,
                                android_glue::MotionAction::Up => TouchPhase::Ended,
                                android_glue::MotionAction::Cancel => TouchPhase::Cancelled,
                            },
                            location: (motion.x as f64, motion.y as f64),
                            id: motion.pointer_id as u64,
                            device_id: DEVICE_ID,
                        }),
                    })
                },
                android_glue::Event::InitWindow => {
                    // The activity went to foreground.
                    if let Some(cb) = self.suspend_callback.borrow().as_ref() {
                        (*cb)(false);
                    }
                    Some(Event::Suspended(false))
                },
                android_glue::Event::TermWindow => {
                    // The activity went to background.
                    if let Some(cb) = self.suspend_callback.borrow().as_ref() {
                        (*cb)(true);
                    }
                    Some(Event::Suspended(true))
                },
                android_glue::Event::WindowResized |
                android_glue::Event::ConfigChanged => {
                    // Activity Orientation changed or resized.
                    let native_window = unsafe { android_glue::get_native_window() };
                    if native_window.is_null() {
                        None
                    } else {
                        let w = unsafe { ffi::ANativeWindow_getWidth(native_window as *const _) } as u32;
                        let h = unsafe { ffi::ANativeWindow_getHeight(native_window as *const _) } as u32;
                        Some(Event::WindowEvent {
                            window_id: RootWindowId(WindowId),
                            event: WindowEvent::Resized(w, h),
                        })
                    }
                },
                android_glue::Event::WindowRedrawNeeded => {
                    // The activity needs to be redrawn.
                    Some(Event::WindowEvent {
                        window_id: RootWindowId(WindowId),
                        event: WindowEvent::Refresh,
                    })
                }
                android_glue::Event::Wake => {
                    Some(Event::Awakened)
                }
                _ => {
                    None
                }
            };

            if let Some(event) = e {
                callback(event);
            }
        };
    }

    pub fn set_suspend_callback(&self, cb: Option<Box<Fn(bool) -> ()>>) {
        *self.suspend_callback.borrow_mut() = cb;
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(::Event) -> ::ControlFlow,
    {
        // Yeah that's a very bad implementation.
        loop {
            let mut control_flow = ::ControlFlow::Continue;
            self.poll_events(|e| {
                if let ::ControlFlow::Break = callback(e) {
                    control_flow = ::ControlFlow::Break;
                }
            });
            if let ::ControlFlow::Break = control_flow {
                break;
            }
            ::std::thread::sleep(::std::time::Duration::from_millis(5));
        }
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy
    }
}

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), ::EventsLoopClosed> {
        android_glue::wake_event_loop();
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

pub struct Window {
    native_window: *const c_void,
}

#[derive(Clone)]
pub struct MonitorId;

mod ffi;

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some("Primary".to_string())
    }

    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        unsafe {
            let window = android_glue::get_native_window();
            (ffi::ANativeWindow_getWidth(window) as u32, ffi::ANativeWindow_getHeight(window) as u32)
        }
    }

    #[inline]
    pub fn get_position(&self) -> (i32, i32) {
        // Android assumes single screen
        (0, 0)
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f32 {
        1.0
    }
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;
#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

impl Window {
    pub fn new(_: &EventsLoop, win_attribs: &WindowAttributes,
               _: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError>
    {
        // not implemented
        assert!(win_attribs.min_dimensions.is_none());
        assert!(win_attribs.max_dimensions.is_none());

        let native_window = unsafe { android_glue::get_native_window() };
        if native_window.is_null() {
            return Err(OsError(format!("Android's native window is null")));
        }

        android_glue::set_multitouch(win_attribs.multitouch);

        Ok(Window {
            native_window: native_window as *const _,
        })
    }

    #[inline]
    pub fn get_native_window(&self) -> *const c_void {
        self.native_window
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
        if self.native_window.is_null() {
            None
        } else {
            Some((
                unsafe { ffi::ANativeWindow_getWidth(self.native_window as *const _) } as u32,
                unsafe { ffi::ANativeWindow_getHeight(self.native_window as *const _) } as u32
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
    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!();
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn set_cursor(&self, _: MouseCursor) {
    }

    #[inline]
    pub fn set_cursor_state(&self, _state: CursorState) -> Result<(), String> {
        Ok(())
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    #[inline]
    pub fn set_cursor_position(&self, _x: i32, _y: i32) -> Result<(), ()> {
        Ok(())
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        // Android has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<RootMonitorId>) {
        // Android has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // N/A
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        RootMonitorId{inner: MonitorId}
    }

    pub fn id(&self) -> WindowId {
        WindowId
    }
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

// Constant device ID, to be removed when this backend is updated to report real device IDs.
const DEVICE_ID: ::DeviceId = ::DeviceId(DeviceId);
