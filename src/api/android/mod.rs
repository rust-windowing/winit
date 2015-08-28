#![cfg(target_os = "android")]

extern crate android_glue;

use libc;
use std::ffi::{CString};
use std::sync::mpsc::{Receiver, channel};
use {CreationError, Event, MouseCursor};
use CreationError::OsError;
use events::ElementState::{Pressed, Released};
use events::Event::{MouseInput, MouseMoved};
use events::MouseButton;

use std::collections::VecDeque;

use Api;
use BuilderAttribs;
use ContextError;
use CursorState;
use GlContext;
use GlRequest;
use PixelFormat;
use native_monitor::NativeMonitorId;

use api::egl;
use api::egl::Context as EglContext;

pub struct Window {
    context: EglContext,
    event_rx: Receiver<android_glue::Event>,
}

#[derive(Clone)]
pub struct MonitorID;

mod ffi;

pub fn get_available_monitors() -> VecDeque<MonitorID> {
    let mut rb = VecDeque::new();
    rb.push_back(MonitorID);
    rb
}

pub fn get_primary_monitor() -> MonitorID {
    MonitorID
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        Some("Primary".to_string())
    }

    pub fn get_native_identifier(&self) -> NativeMonitorId {
        NativeMonitorId::Unavailable
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!()
    }
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        match self.window.event_rx.try_recv() {
            Ok(event) => {
                match event {
                    android_glue::Event::EventDown => Some(MouseInput(Pressed, MouseButton::Left)),
                    android_glue::Event::EventUp => Some(MouseInput(Released, MouseButton::Left)),
                    android_glue::Event::EventMove(x, y) => Some(MouseMoved((x as i32, y as i32))),
                    _ => None,
                }
            }
            Err(_) => {
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
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        use std::{mem, ptr};

        let native_window = unsafe { android_glue::get_native_window() };
        if native_window.is_null() {
            return Err(OsError(format!("Android's native window is null")));
        }

        let context = try!(EglContext::new(egl::ffi::egl::Egl, &builder, egl::NativeDisplay::Android)
                                                .and_then(|p| p.finish(native_window as *const _)));

        let (tx, rx) = channel();
        android_glue::add_sender(tx);

        Ok(Window {
            context: context,
            event_rx: rx,
        })
    }

    pub fn is_closed(&self) -> bool {
        false
    }

    pub fn set_title(&self, _: &str) {
    }

    pub fn show(&self) {
    }

    pub fn hide(&self) {
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        None
    }

    pub fn set_position(&self, _x: i32, _y: i32) {
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        let native_window = unsafe { android_glue::get_native_window() };

        if native_window.is_null() {
            None
        } else {
            Some((
                unsafe { ffi::ANativeWindow_getWidth(native_window) } as u32,
                unsafe { ffi::ANativeWindow_getHeight(native_window) } as u32
            ))
        }
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.get_inner_size()
    }

    pub fn set_inner_size(&self, _x: u32, _y: u32) {
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self
        }
    }

    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self
        }
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!();
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!();
    }

    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    pub fn set_cursor(&self, _: MouseCursor) {
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        Ok(())
    }

    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        unimplemented!();
    }
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl GlContext for Window {
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.context.make_current()
    }

    fn is_current(&self) -> bool {
        self.context.is_current()
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        self.context.get_proc_address(addr)
    }

    fn swap_buffers(&self) -> Result<(), ContextError> {
        self.context.swap_buffers()
    }

    fn get_api(&self) -> Api {
        self.context.get_api()
    }

    fn get_pixel_format(&self) -> PixelFormat {
        self.context.get_pixel_format()
    }
}

#[cfg(feature = "window")]
#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        unimplemented!()
    }
}

pub struct HeadlessContext(EglContext);

impl HeadlessContext {
    /// See the docs in the crate root file.
    pub fn new(builder: BuilderAttribs) -> Result<HeadlessContext, CreationError> {
        let context = try!(EglContext::new(egl::ffi::egl::Egl, &builder, egl::NativeDisplay::Android));
        let context = try!(context.finish_pbuffer());
        Ok(HeadlessContext(context))
    }
}

unsafe impl Send for HeadlessContext {}
unsafe impl Sync for HeadlessContext {}

impl GlContext for HeadlessContext {
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.0.make_current()
    }

    fn is_current(&self) -> bool {
        self.0.is_current()
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        self.0.get_proc_address(addr)
    }

    fn swap_buffers(&self) -> Result<(), ContextError> {
        self.0.swap_buffers()
    }

    fn get_api(&self) -> Api {
        self.0.get_api()
    }

    fn get_pixel_format(&self) -> PixelFormat {
        self.0.get_pixel_format()
    }
}
