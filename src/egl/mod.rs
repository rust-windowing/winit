use {Event, Hints, MonitorID};

mod ffi;

pub struct Window {
    display: ffi::EGLDisplay,
    context: ffi::EGLContext,
    //surface: ffi::EGLSurface,
}

impl Window {
    pub fn new(_dimensions: Option<(uint, uint)>, _title: &str,
        _hints: &Hints, _monitor: Option<MonitorID>)
        -> Result<Window, String>
    {
        use std::{mem, ptr};

        let display = unsafe {
            let display = ffi::eglGetDisplay(mem::transmute(ffi::EGL_DEFAULT_DISPLAY));
            if display.is_null() {
                return Err("No EGL display connection available".to_string());
            }
            display
        };

        let (_major, _minor) = unsafe {
            let mut major: ffi::EGLint = mem::uninitialized();
            let mut minor: ffi::EGLint = mem::uninitialized();

            if ffi::eglInitialize(display, &mut major, &mut minor) != ffi::EGL_TRUE {
                return Err(format!("eglInitialize failed"))
            }

            (major, minor)
        };

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

        let context = unsafe {
            let context = ffi::eglCreateContext(display, config, ptr::null(), ptr::null());
            if context.is_null() {
                return Err(format!("eglCreateContext failed"))
            }
            context
        };

        /*let surface = unsafe {
            let surface = ffi::eglCreateWindowSurface(display, config, native_window, ptr::null());
            if surface.is_null() {
                return Err(format!("eglCreateWindowSurface failed"))
            }
            surface
        };*/

        Ok(Window {
            display: display,
            context: context,
            //surface: surface,
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

    pub fn set_position(&self, _x: uint, _y: uint) {
    }

    pub fn get_inner_size(&self) -> Option<(uint, uint)> {
        unimplemented!()
    }

    pub fn get_outer_size(&self) -> Option<(uint, uint)> {
        unimplemented!()
    }

    pub fn set_inner_size(&self, _x: uint, _y: uint) {
    }

    pub fn poll_events(&self) -> Vec<Event> {
        Vec::new()
    }

    pub fn wait_events(&self) -> Vec<Event> {
        Vec::new()
    }

    pub fn make_current(&self) {
        unimplemented!()
        /*unsafe {
            ffi::eglMakeCurrent(self.display, self.surface, self.surface, self.context);
        }*/
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
        unimplemented!()
        /*unsafe {
            ffi::eglSwapBuffers(self.display, self.surface);
        }*/
    }
}

#[unsafe_destructor]
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            //ffi::eglDestroySurface(self.display, self.surface);
            ffi::eglDestroyContext(self.display, self.context);
            ffi::eglTerminate(self.display);
        }
    }
}
