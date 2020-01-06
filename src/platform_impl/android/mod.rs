#![cfg(target_os = "android")]

extern crate android_glue;

mod ffi;

use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt,
    os::raw::c_void,
    sync::mpsc::{channel, Receiver},
};

use crate::{
    error::{ExternalError, NotSupportedError},
    events::{Touch, TouchPhase},
    window::MonitorHandle as RootMonitorHandle,
    CreationError, CursorIcon, Event, LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize,
    WindowAttributes, WindowEvent, WindowId as RootWindowId,
};
use raw_window_handle::{android::AndroidHandle, RawWindowHandle};
use CreationError::OsError;

pub type OsError = std::io::Error;

pub struct EventLoop {
    event_rx: Receiver<android_glue::Event>,
    suspend_callback: RefCell<Option<Box<dyn Fn(bool) -> ()>>>,
}

#[derive(Clone)]
pub struct EventLoopProxy;

impl EventLoop {
    pub fn new() -> EventLoop {
        let (tx, rx) = channel();
        android_glue::add_sender(tx);
        EventLoop {
            event_rx: rx,
            suspend_callback: Default::default(),
        }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut rb = VecDeque::with_capacity(1);
        rb.push_back(MonitorHandle);
        rb
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
    where
        F: FnMut(::Event),
    {
        while let Ok(event) = self.event_rx.try_recv() {
            let e = match event {
                android_glue::Event::EventMotion(motion) => {
                    let dpi_factor = MonitorHandle.scale_factor();
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
                            force: None, // TODO
                            id: motion.pointer_id as u64,
                            device_id: DEVICE_ID,
                        }),
                    })
                }
                android_glue::Event::InitWindow => {
                    // The activity went to foreground.
                    if let Some(cb) = self.suspend_callback.borrow().as_ref() {
                        (*cb)(false);
                    }
                    Some(Event::Resumed)
                }
                android_glue::Event::TermWindow => {
                    // The activity went to background.
                    if let Some(cb) = self.suspend_callback.borrow().as_ref() {
                        (*cb)(true);
                    }
                    Some(Event::Suspended)
                }
                android_glue::Event::WindowResized | android_glue::Event::ConfigChanged => {
                    // Activity Orientation changed or resized.
                    let native_window = unsafe { android_glue::native_window() };
                    if native_window.is_null() {
                        None
                    } else {
                        let dpi_factor = MonitorHandle.scale_factor();
                        let physical_size = MonitorHandle.size();
                        let size = LogicalSize::from_physical(physical_size, dpi_factor);
                        Some(Event::WindowEvent {
                            window_id: RootWindowId(WindowId),
                            event: WindowEvent::Resized(size),
                        })
                    }
                }
                android_glue::Event::WindowRedrawNeeded => {
                    // The activity needs to be redrawn.
                    Some(Event::WindowEvent {
                        window_id: RootWindowId(WindowId),
                        event: WindowEvent::Redraw,
                    })
                }
                android_glue::Event::Wake => Some(Event::Awakened),
                _ => None,
            };

            if let Some(event) = e {
                callback(event);
            }
        }
    }

    pub fn set_suspend_callback(&self, cb: Option<Box<dyn Fn(bool) -> ()>>) {
        *self.suspend_callback.borrow_mut() = cb;
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
    where
        F: FnMut(::Event) -> ::ControlFlow,
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

    pub fn create_proxy(&self) -> EventLoopProxy {
        EventLoopProxy
    }
}

impl EventLoopProxy {
    pub fn wakeup(&self) -> Result<(), ::EventLoopClosed<()>> {
        android_glue::wake_event_loop();
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId;

impl WindowId {
    pub unsafe fn dummy() -> Self {
        WindowId
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        DeviceId
    }
}

pub struct Window {
    native_window: *const c_void,
}

#[derive(Clone)]
pub struct MonitorHandle;

impl fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct MonitorHandle {
            name: Option<String>,
            dimensions: PhysicalSize<u32>,
            position: PhysicalPosition<i32>,
            scale_factor: f64,
        }

        let monitor_id_proxy = MonitorHandle {
            name: self.name(),
            dimensions: self.size(),
            position: self.outer_position(),
            scale_factor: self.scale_factor(),
        };

        monitor_id_proxy.fmt(f)
    }
}

impl MonitorHandle {
    #[inline]
    pub fn name(&self) -> Option<String> {
        Some("Primary".to_string())
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        unsafe {
            let window = android_glue::native_window();
            (
                ffi::ANativeWindow_getWidth(window) as f64,
                ffi::ANativeWindow_getHeight(window) as f64,
            )
                .into()
        }
    }

    #[inline]
    pub fn outer_position(&self) -> PhysicalPosition<i32> {
        // Android assumes single screen
        (0, 0).into()
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        1.0
    }
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;
#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

impl Window {
    pub fn new(
        _: &EventLoop,
        win_attribs: WindowAttributes,
        _: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window, CreationError> {
        let native_window = unsafe { android_glue::native_window() };
        if native_window.is_null() {
            return Err(OsError(format!("Android's native window is null")));
        }

        android_glue::set_multitouch(true);

        Ok(Window {
            native_window: native_window as *const _,
        })
    }

    #[inline]
    pub fn native_window(&self) -> *const c_void {
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
    pub fn outer_position(&self) -> Option<LogicalPosition<f64>> {
        // N/A
        None
    }

    #[inline]
    pub fn inner_position(&self) -> Option<LogicalPosition<f64>> {
        // N/A
        None
    }

    #[inline]
    pub fn set_outer_position(&self, _position: LogicalPosition<f64>) {
        // N/A
    }

    #[inline]
    pub fn set_min_inner_size(&self, _dimensions: Option<LogicalSize<f64>>) {
        // N/A
    }

    #[inline]
    pub fn set_max_inner_size(&self, _dimensions: Option<LogicalSize<f64>>) {
        // N/A
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // N/A
    }

    #[inline]
    pub fn inner_size(&self) -> Option<LogicalSize<f64>> {
        if self.native_window.is_null() {
            None
        } else {
            let dpi_factor = self.scale_factor();
            let physical_size = self.current_monitor().size();
            Some(LogicalSize::from_physical(physical_size, dpi_factor))
        }
    }

    #[inline]
    pub fn outer_size(&self) -> Option<LogicalSize<f64>> {
        self.inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, _size: LogicalSize<f64>) {
        // N/A
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.current_monitor().scale_factor()
    }

    #[inline]
    pub fn set_cursor_icon(&self, _: CursorIcon) {
        // N/A
    }

    #[inline]
    pub fn set_cursor_grab(&self, _grab: bool) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn hide_cursor(&self, _hide: bool) {
        // N/A
    }

    #[inline]
    pub fn set_cursor_position(
        &self,
        _position: LogicalPosition<f64>,
    ) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn set_minimized(&self, _minimized: bool) {
        unimplemented!()
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        // N/A
        // Android has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<RootMonitorHandle> {
        // N/A
        // Android has single screen maximized apps so nothing to do
        None
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<RootMonitorHandle>) {
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
    pub fn set_ime_position(&self, _spot: LogicalPosition<f64>) {
        // N/A
    }

    #[inline]
    pub fn current_monitor(&self) -> RootMonitorHandle {
        RootMonitorHandle {
            inner: MonitorHandle,
        }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut rb = VecDeque::with_capacity(1);
        rb.push_back(MonitorHandle);
        rb
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId
    }

    #[inline]
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let handle = AndroidHandle {
            a_native_window: self.native_window,
            ..WindowsHandle::empty()
        };
        RawWindowHandle::Android(handle)
    }
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

// Constant device ID, to be removed when this backend is updated to report real device IDs.
const DEVICE_ID: ::DeviceId = ::DeviceId(DeviceId);
