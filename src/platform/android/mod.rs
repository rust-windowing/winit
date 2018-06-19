#![cfg(target_os = "android")]

extern crate android_glue;

mod ffi;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::os::raw::c_void;
use std::sync::mpsc::{Receiver, channel};

use {
    CreationError,
    Event,
    LogicalPosition,
    LogicalSize,
    MouseCursor,
    PhysicalPosition,
    PhysicalSize,
    WindowAttributes,
    WindowEvent,
    WindowId as RootWindowId,
};
use CreationError::OsError;
use events::{Touch, TouchPhase};
use window::MonitorId as RootMonitorId;

pub struct EventsLoop {
    event_rx: Receiver<android_glue::Event>,
    suspend_callback: RefCell<Option<Box<Fn(bool) -> ()>>>,
}

#[derive(Clone)]
pub struct EventsLoopProxy;

impl EventsLoop {
    pub fn new() -> EventsLoop {
        let (tx, rx) = channel();
        android_glue::add_sender(tx);
        EventsLoop {
            event_rx: rx,
            suspend_callback: Default::default(),
        }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut rb = VecDeque::with_capacity(1);
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
                    let dpi_factor = MonitorId.get_hidpi_factor();
                    let location = LogicalPosition::from_physical(
                        (motion.x as f64, motion.y as f64),
                        dpi_factor,
                    );
                    Some(Event::WindowEvent {
                        window_id: RootWindowId(WindowId),
                        event: WindowEvent::Touch(Touch {
                            phase: match motion.action {
                                android_glue::MotionAction::Down => TouchPhase::Started,
                                android_glue::MotionAction::Move => TouchPhase::Moved,
                                android_glue::MotionAction::Up => TouchPhase::Ended,
                                android_glue::MotionAction::Cancel => TouchPhase::Cancelled,
                            },
                            location,
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
                        let dpi_factor = MonitorId.get_hidpi_factor();
                        let physical_size = MonitorId.get_dimensions();
                        let size = LogicalSize::from_physical(physical_size, dpi_factor);
                        Some(Event::WindowEvent {
                            window_id: RootWindowId(WindowId),
                            event: WindowEvent::Resized(size),
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

impl fmt::Debug for MonitorId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct MonitorId {
            name: Option<String>,
            dimensions: PhysicalSize,
            position: PhysicalPosition,
            hidpi_factor: f64,
        }

        let monitor_id_proxy = MonitorId {
            name: self.get_name(),
            dimensions: self.get_dimensions(),
            position: self.get_position(),
            hidpi_factor: self.get_hidpi_factor(),
        };

        monitor_id_proxy.fmt(f)
    }
}

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some("Primary".to_string())
    }

    #[inline]
    pub fn get_dimensions(&self) -> PhysicalSize {
        unsafe {
            let window = android_glue::get_native_window();
            (
                ffi::ANativeWindow_getWidth(window) as f64,
                ffi::ANativeWindow_getHeight(window) as f64,
            ).into()
        }
    }

    #[inline]
    pub fn get_position(&self) -> PhysicalPosition {
        // Android assumes single screen
        (0, 0).into()
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        1.0
    }
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;
#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

impl Window {
    pub fn new(_: &EventsLoop, win_attribs: WindowAttributes,
               _: PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError>
    {
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
        // N/A
    }

    #[inline]
    pub fn show(&self) {
        // N/A
    }

    #[inline]
    pub fn hide(&self) {
        // N/A
    }

    #[inline]
    pub fn get_position(&self) -> Option<LogicalPosition> {
        // N/A
        None
    }

    #[inline]
    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        // N/A
        None
    }

    #[inline]
    pub fn set_position(&self, _position: LogicalPosition) {
        // N/A
    }

    #[inline]
    pub fn set_min_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // N/A
    }

    #[inline]
    pub fn set_max_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // N/A
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // N/A
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        if self.native_window.is_null() {
            None
        } else {
            let dpi_factor = self.get_hidpi_factor();
            let physical_size = self.get_current_monitor().get_dimensions();
            Some(LogicalSize::from_physical(physical_size, dpi_factor))
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        self.get_inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, _size: LogicalSize) {
        // N/A
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        self.get_current_monitor().get_hidpi_factor()
    }

    #[inline]
    pub fn set_cursor(&self, _: MouseCursor) {
        // N/A
    }

    #[inline]
    pub fn grab_cursor(&self, _grab: bool) -> Result<(), String> {
        Err("Cursor grabbing is not possible on Android.".to_owned())
    }

    #[inline]
    pub fn hide_cursor(&self, _hide: bool) {
        // N/A
    }

    #[inline]
    pub fn set_cursor_position(&self, _position: LogicalPosition) -> Result<(), String> {
        Err("Setting cursor position is not possible on Android.".to_owned())
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        // N/A
        // Android has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<RootMonitorId>) {
        // N/A
        // Android has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // N/A
    }

    #[inline]
    pub fn set_always_on_top(&self, _always_on_top: bool) {
        // N/A
    }

    #[inline]
    pub fn set_window_icon(&self, _icon: Option<::Icon>) {
        // N/A
    }

    #[inline]
    pub fn set_ime_spot(&self, _spot: LogicalPosition) {
        // N/A
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        RootMonitorId { inner: MonitorId }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut rb = VecDeque::with_capacity(1);
        rb.push_back(MonitorId);
        rb
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        MonitorId
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId
    }
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

// Constant device ID, to be removed when this backend is updated to report real device IDs.
const DEVICE_ID: ::DeviceId = ::DeviceId(DeviceId);
