#![cfg(all(target_os = "windows", target_os = "linux"))]        // always false of the moment

use BuilderAttribs;
use CreationError;
use GlRequest;
use Api;

use libc;
use std::ffi::CString;
use std::{mem, ptr};

pub mod ffi;

pub struct Context {
    egl: ffi::egl::Egl,
    display: ffi::egl::types::EGLDisplay,
    context: ffi::egl::types::EGLContext,
    surface: ffi::egl::types::EGLSurface,
}

impl Context {
    pub fn new(egl: ffi::egl::Egl, builder: BuilderAttribs,
               native_display: Option<ffi::EGLNativeDisplayType>,
               native_window: ffi::EGLNativeWindowType) -> Result<Context, CreationError>
    {
        if builder.sharing.is_some() {
            unimplemented!()
        }

        let display = unsafe {
            let display = egl.GetDisplay(native_display.unwrap_or(mem::transmute(ffi::egl::DEFAULT_DISPLAY)));
            if display.is_null() {
                return Err(CreationError::OsError("No EGL display connection available".to_string()));
            }
            display
        };

        let (_major, _minor) = unsafe {
            let mut major: ffi::egl::types::EGLint = mem::uninitialized();
            let mut minor: ffi::egl::types::EGLint = mem::uninitialized();

            if egl.Initialize(display, &mut major, &mut minor) == 0 {
                return Err(CreationError::OsError(format!("eglInitialize failed")))
            }

            (major, minor)
        };

        let use_gles2 = match builder.gl_version {
            GlRequest::Specific(Api::OpenGlEs, (2, _)) => true,
            GlRequest::Specific(Api::OpenGlEs, _) => false,
            GlRequest::Specific(_, _) => return Err(CreationError::NotSupported),
            GlRequest::GlThenGles { opengles_version: (2, _), .. } => true,
            _ => false,
        };

        let mut attribute_list = vec!();
        attribute_list.push(ffi::egl::RENDERABLE_TYPE as i32);
        attribute_list.push(ffi::egl::OPENGL_ES2_BIT as i32);

        attribute_list.push(ffi::egl::CONFORMANT as i32);
        attribute_list.push(ffi::egl::OPENGL_ES2_BIT as i32);

        attribute_list.push(ffi::egl::SURFACE_TYPE as i32);
        attribute_list.push(ffi::egl::WINDOW_BIT as i32);

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
            if egl.ChooseConfig(display, attribute_list.as_ptr(), &mut config, 1,
                                &mut num_config) == 0
            {
                return Err(CreationError::OsError(format!("eglChooseConfig failed")))
            }

            if num_config <= 0 {
                return Err(CreationError::OsError(format!("eglChooseConfig returned no available config")))
            }

            config
        };

        let surface = unsafe {
            let surface = egl.CreateWindowSurface(display, config, native_window, ptr::null());
            if surface.is_null() {
                return Err(CreationError::OsError(format!("eglCreateWindowSurface failed")))
            }
            surface
        };

        let context = unsafe {
            let mut context_attributes = vec!();
            context_attributes.push(ffi::egl::CONTEXT_CLIENT_VERSION as i32);
            context_attributes.push(2);
            context_attributes.push(ffi::egl::NONE as i32);

            let context = egl.CreateContext(display, config, ptr::null(),
                                            context_attributes.as_ptr());
            if context.is_null() {
                return Err(CreationError::OsError(format!("eglCreateContext failed")))
            }
            context
        };

        Ok(Context {
            egl: egl,
            display: display,
            context: context,
            surface: surface,
        })
    }

    pub fn make_current(&self) {
        let ret = unsafe {
            self.egl.MakeCurrent(self.display, self.surface, self.surface, self.context)
        };

        if ret == 0 {
            panic!("eglMakeCurrent failed");
        }
    }

    pub fn is_current(&self) -> bool {
        unsafe { self.egl.GetCurrentContext() == self.context }
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        unsafe {
            self.egl.GetProcAddress(addr) as *const ()
        }
    }

    pub fn swap_buffers(&self) {
        let ret = unsafe {
            self.egl.SwapBuffers(self.display, self.surface)
        };

        if ret == 0 {
            panic!("eglSwapBuffers failed");
        }
    }

    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGlEs
    }
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

impl Drop for Context {
    fn drop(&mut self) {
        use std::ptr;

        unsafe {
            // we don't call MakeCurrent(0, 0) because we are not sure that the context
            // is still the current one
            self.egl.DestroyContext(self.display, self.context);
            self.egl.DestroySurface(self.display, self.surface);
            self.egl.Terminate(self.display);
        }
    }
}
