extern crate android_glue;

use libc;
use {CreationError, Event, WindowBuilder};
use CreationError::OsError;
use events::ElementState::{Pressed, Released};
use events::Event::{MouseInput, MouseMoved};
use events::MouseButton::LeftMouseButton;

#[cfg(feature = "headless")]
use HeadlessRendererBuilder;

pub struct Window {
    display: ffi::egl::types::EGLDisplay,
    context: ffi::egl::types::EGLContext,
    surface: ffi::egl::types::EGLSurface,
    event_rx: Receiver<android_glue::Event>,
}

pub struct MonitorID;

mod ffi;

pub fn get_available_monitors() -> Vec<MonitorID> {
    vec![ MonitorID ]
}

pub fn get_primary_monitor() -> MonitorID {
    MonitorID
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        Some("Primary".to_string())
    }

    pub fn get_dimensions(&self) -> (uint, uint) {
        unimplemented!()
    }
}

#[cfg(feature = "headless")]
pub struct HeadlessContext(int);

#[cfg(feature = "headless")]
impl HeadlessContext {
    /// See the docs in the crate root file.
    pub fn new(_builder: HeadlessRendererBuilder) -> Result<HeadlessContext, CreationError> {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub unsafe fn make_current(&self) {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub fn get_proc_address(&self, _addr: &str) -> *const () {
        unimplemented!()
    }
}

impl Window {
    pub fn new(builder: WindowBuilder) -> Result<Window, CreationError> {
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
            Some((2, 0)) => true,
            _ => false,
        };

        let config = unsafe {
            let mut attribute_list = vec!();
            if use_gles2 {
                attribute_list.push_all(&[ffi::egl::RENDERABLE_TYPE as i32,
                                         ffi::egl::OPENGL_ES2_BIT as i32]);
            }
            attribute_list.push_all(&[ffi::egl::RED_SIZE as i32, 1]);
            attribute_list.push_all(&[ffi::egl::GREEN_SIZE as i32, 1]);
            attribute_list.push_all(&[ffi::egl::BLUE_SIZE as i32, 1]);
            attribute_list.push(ffi::egl::NONE as i32);

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
                context_attributes.push_all(&[ffi::egl::CONTEXT_CLIENT_VERSION as i32, 2]);
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

    pub fn get_position(&self) -> Option<(int, int)> {
        None
    }

    pub fn set_position(&self, _x: int, _y: int) {
    }

    pub fn get_inner_size(&self) -> Option<(uint, uint)> {
        let native_window = unsafe { android_glue::get_native_window() };

        if native_window.is_null() {
            None
        } else {
            Some((
                unsafe { ffi::ANativeWindow_getWidth(native_window) } as uint,
                unsafe { ffi::ANativeWindow_getHeight(native_window) } as uint
            ))
        }
    }

    pub fn get_outer_size(&self) -> Option<(uint, uint)> {
        self.get_inner_size()
    }

    pub fn set_inner_size(&self, _x: uint, _y: uint) {
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    pub fn poll_events(&self) -> Vec<Event> {
        use std::time::Duration;
        use std::io::timer;
        timer::sleep(Duration::milliseconds(16));
        Vec::new()
    }

    pub fn wait_events(&self) -> Vec<Event> {
        use std::time::Duration;
        use std::io::timer;
        timer::sleep(Duration::milliseconds(16));
        let mut events = Vec::new();
        loop {
            match self.event_rx.try_recv() {
                Ok(event) => match event {
                    android_glue::Event::EventDown => {
                        events.push(MouseInput(Pressed, LeftMouseButton));
                    },
                    android_glue::Event::EventUp => {
                        events.push(MouseInput(Released, LeftMouseButton));
                    },
                    android_glue::Event::EventMove(x, y) => {
                        events.push(MouseMoved((x as int, y as int)));
                    },
                },
                Err(_) => {
                    break;
                },
            }
        }
        events
    }

    pub fn make_current(&self) {
        unsafe {
            ffi::egl::MakeCurrent(self.display, self.surface, self.surface, self.context);
        }
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        use std::c_str::ToCStr;

        unsafe {
            addr.with_c_str(|s| {
                ffi::egl::GetProcAddress(s) as *const ()
            })
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

    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGlEs
    }

    pub fn set_window_resize_callback(&mut self, _: Option<fn(uint, uint)>) {
    }
}

#[cfg(feature = "window")]
#[deriving(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        unimplemented!()
    }
}

#[unsafe_destructor]
impl Drop for Window {
    fn drop(&mut self) {
        use std::ptr;

        unsafe {
            android_glue::write_log("Destroying gl-init window");
            ffi::egl::MakeCurrent(self.display, ptr::null(), ptr::null(), ptr::null());
            ffi::egl::DestroySurface(self.display, self.surface);
            ffi::egl::DestroyContext(self.display, self.context);
            ffi::egl::Terminate(self.display);
        }
    }
}
