extern crate android_glue;
extern crate native;

use libc;
use self::native::NativeTaskBuilder;
use {Event, WindowBuilder};

pub struct Window {
    display: ffi::EGLDisplay,
    context: ffi::EGLContext,
    surface: ffi::EGLSurface,
}

pub struct MonitorID;

mod ffi;

compile_warning!("The Android implementation is not fully working yet")

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

impl Window {
    pub fn new(_builder: WindowBuilder) -> Result<Window, String> {
        use std::{mem, ptr};
        use std::task::TaskBuilder;

        let native_window = unsafe { android_glue::get_native_window() };
        if native_window.is_null() {
            return Err(format!("Android's native window is null"));
        }

        let display = unsafe {
            let display = ffi::eglGetDisplay(mem::transmute(ffi::EGL_DEFAULT_DISPLAY));
            if display.is_null() {
                return Err("No EGL display connection available".to_string());
            }
            display
        };

        android_glue::write_log("eglGetDisplay succeeded");

        let (_major, _minor) = unsafe {
            let mut major: ffi::EGLint = mem::uninitialized();
            let mut minor: ffi::EGLint = mem::uninitialized();

            if ffi::eglInitialize(display, &mut major, &mut minor) != ffi::EGL_TRUE {
                return Err(format!("eglInitialize failed"))
            }

            (major, minor)
        };

        android_glue::write_log("eglInitialize succeeded");

        let config = unsafe {
            let attribute_list = [
                ffi::EGL_RED_SIZE, 1,
                ffi::EGL_GREEN_SIZE, 1,
                ffi::EGL_BLUE_SIZE, 1,
                ffi::EGL_NONE
            ];

            let mut num_config: ffi::EGLint = mem::uninitialized();
            let mut config: ffi::EGLConfig = mem::uninitialized();
            if ffi::eglChooseConfig(display, attribute_list.as_ptr(), &mut config, 1,
                &mut num_config) != ffi::EGL_TRUE
            {
                return Err(format!("eglChooseConfig failed"))
            }

            if num_config <= 0 {
                return Err(format!("eglChooseConfig returned no available config"))
            }

            config
        };

        android_glue::write_log("eglChooseConfig succeeded");

        let context = unsafe {
            let context = ffi::eglCreateContext(display, config, ptr::null(), ptr::null());
            if context.is_null() {
                return Err(format!("eglCreateContext failed"))
            }
            context
        };

        android_glue::write_log("eglCreateContext succeeded");

        let surface = unsafe {
            let surface = ffi::eglCreateWindowSurface(display, config, native_window, ptr::null());
            if surface.is_null() {
                return Err(format!("eglCreateWindowSurface failed"))
            }
            surface
        };
        
        android_glue::write_log("eglCreateWindowSurface succeeded");

        Ok(Window {
            display: display,
            context: context,
            surface: surface,
        })
    }

    pub fn is_closed(&self) -> bool {
        false
    }

    pub fn set_title(&self, _: &str) {
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
        Vec::new()
    }

    pub fn make_current(&self) {
        unsafe {
            ffi::eglMakeCurrent(self.display, self.surface, self.surface, self.context);
        }
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        use std::c_str::ToCStr;

        unsafe {
            addr.with_c_str(|s| {
                ffi::eglGetProcAddress(s) as *const ()
            })
        }
    }

    pub fn swap_buffers(&self) {
        unsafe {
            ffi::eglSwapBuffers(self.display, self.surface);
        }
    }
}

#[unsafe_destructor]
impl Drop for Window {
    fn drop(&mut self) {
        use std::ptr;

        unsafe {
            android_glue::write_log("Destroying gl-init window");
            ffi::eglMakeCurrent(self.display, ptr::null(), ptr::null(), ptr::null());
            ffi::eglDestroySurface(self.display, self.surface);
            ffi::eglDestroyContext(self.display, self.context);
            ffi::eglTerminate(self.display);
        }
    }
}
