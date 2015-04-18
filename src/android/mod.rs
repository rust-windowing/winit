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
use CursorState;
use GlRequest;
use PixelFormat;
use native_monitor::NativeMonitorId;

pub struct Window {
    display: ffi::egl::types::EGLDisplay,
    context: ffi::egl::types::EGLContext,
    surface: ffi::egl::types::EGLSurface,
    event_rx: Receiver<android_glue::Event>,
}

pub struct MonitorID;

mod ffi;

pub fn get_available_monitors() -> VecDeque <MonitorID> {
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

#[cfg(feature = "headless")]
pub struct HeadlessContext(i32);

#[cfg(feature = "headless")]
impl HeadlessContext {
    /// See the docs in the crate root file.
    pub fn new(_builder: BuilderAttribs) -> Result<HeadlessContext, CreationError> {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub unsafe fn make_current(&self) {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub fn is_current(&self) -> bool {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub fn get_proc_address(&self, _addr: &str) -> *const () {
        unimplemented!()
    }

    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGlEs
    }
}

#[cfg(feature = "headless")]
unsafe impl Send for HeadlessContext {}
#[cfg(feature = "headless")]
unsafe impl Sync for HeadlessContext {}

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

        if builder.sharing.is_some() {
            unimplemented!()
        }

        let native_window = unsafe { android_glue::get_native_window() };
        if native_window.is_null() {
            return Err(OsError(format!("Android's native window is null")));
        }

        let display = unsafe {
            let display = ffi::egl::GetDisplay(mem::transmute(ffi::egl::DEFAULT_DISPLAY));
            if display.is_null() {
                return Err(OsError("No EGL display connection available".to_string()));
            }
            display
        };

        android_glue::write_log("eglGetDisplay succeeded");

        let (_major, _minor) = unsafe {
            let mut major: ffi::egl::types::EGLint = mem::uninitialized();
            let mut minor: ffi::egl::types::EGLint = mem::uninitialized();

            if ffi::egl::Initialize(display, &mut major, &mut minor) == 0 {
                return Err(OsError(format!("eglInitialize failed")))
            }

            (major, minor)
        };

        android_glue::write_log("eglInitialize succeeded");

        let use_gles2 = match builder.gl_version {
            GlRequest::Specific(Api::OpenGlEs, (2, _)) => true,
            GlRequest::Specific(Api::OpenGlEs, _) => false,
            GlRequest::Specific(_, _) => panic!("Only OpenGL ES is supported"),     // FIXME: return a result
            GlRequest::GlThenGles { opengles_version: (2, _), .. } => true,
            _ => false,
        };

        let mut attribute_list = vec!();

        if use_gles2 {
            attribute_list.push(ffi::egl::RENDERABLE_TYPE as i32);
            attribute_list.push(ffi::egl::OPENGL_ES2_BIT as i32);
        }

        {
            let (red, green, blue) = match builder.color_bits.unwrap_or(24) {
                24 => (8, 8, 8),
                16 => (6, 5, 6),
                _ => panic!("Bad color_bits"),
            };
            attribute_list.push(ffi::egl::RED_SIZE as i32);
            attribute_list.push(red);
            attribute_list.push(ffi::egl::GREEN_SIZE as i32);
            attribute_list.push(green);
            attribute_list.push(ffi::egl::BLUE_SIZE as i32);
            attribute_list.push(blue);
        }

        attribute_list.push(ffi::egl::DEPTH_SIZE as i32);
        attribute_list.push(builder.depth_bits.unwrap_or(8) as i32);

        attribute_list.push(ffi::egl::NONE as i32);

        let config = unsafe {
            let mut num_config: ffi::egl::types::EGLint = mem::uninitialized();
            let mut config: ffi::egl::types::EGLConfig = mem::uninitialized();
            if ffi::egl::ChooseConfig(display, attribute_list.as_ptr(), &mut config, 1,
                &mut num_config) == 0
            {
                return Err(OsError(format!("eglChooseConfig failed")))
            }

            if num_config <= 0 {
                return Err(OsError(format!("eglChooseConfig returned no available config")))
            }

            config
        };

        android_glue::write_log("eglChooseConfig succeeded");

        let context = unsafe {
            let mut context_attributes = vec!();
            if use_gles2 {
                context_attributes.push(ffi::egl::CONTEXT_CLIENT_VERSION as i32);
                context_attributes.push(2);
            }
            context_attributes.push(ffi::egl::NONE as i32);

            let context = ffi::egl::CreateContext(display, config, ptr::null(),
                                                  context_attributes.as_ptr());
            if context.is_null() {
                return Err(OsError(format!("eglCreateContext failed")))
            }
            context
        };

        android_glue::write_log("eglCreateContext succeeded");

        let surface = unsafe {
            let surface = ffi::egl::CreateWindowSurface(display, config, native_window, ptr::null());
            if surface.is_null() {
                return Err(OsError(format!("eglCreateWindowSurface failed")))
            }
            surface
        };

        android_glue::write_log("eglCreateWindowSurface succeeded");

        let (tx, rx) = channel();
        android_glue::add_sender(tx);

        Ok(Window {
            display: display,
            context: context,
            surface: surface,
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

    pub fn make_current(&self) {
        unsafe {
            ffi::egl::MakeCurrent(self.display, self.surface, self.surface, self.context);
        }
    }

    pub fn is_current(&self) -> bool {
        unsafe { ffi::egl::GetCurrentContext() == self.context }
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        unsafe {
            ffi::egl::GetProcAddress(addr) as *const ()
        }
    }

    pub fn swap_buffers(&self) {
        unsafe {
            ffi::egl::SwapBuffers(self.display, self.surface);
        }
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        self.display as *mut libc::c_void
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGlEs
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

#[cfg(feature = "window")]
#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        unimplemented!()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        use std::ptr;

        unsafe {
            // we don't call MakeCurrent(0, 0) because we are not sure that the context
            // is still the current one
            android_glue::write_log("Destroying gl-init window");
            ffi::egl::DestroySurface(self.display, self.surface);
            ffi::egl::DestroyContext(self.display, self.context);
            ffi::egl::Terminate(self.display);
        }
    }
}
