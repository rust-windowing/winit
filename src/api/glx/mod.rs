#![cfg(all(target_os = "linux", feature = "window"))]

use BuilderAttribs;
use CreationError;
use GlContext;
use GlRequest;
use Api;
use PixelFormat;

use libc;
use std::ffi::CString;
use std::{mem, ptr};

use api::x11::ffi;

pub struct Context {
    display: *mut ffi::Display,
    window: ffi::Window,
    context: ffi::GLXContext,
}

// TODO: remove me
fn with_c_str<F, T>(s: &str, f: F) -> T where F: FnOnce(*const libc::c_char) -> T {
    use std::ffi::CString;
    let c_str = CString::new(s.as_bytes().to_vec()).unwrap();
    f(c_str.as_ptr())
}

impl Context {
    pub fn new(builder: BuilderAttribs, display: *mut ffi::Display, window: ffi::Window,
               fb_config: ffi::glx::types::GLXFBConfig, mut visual_infos: ffi::glx::types::XVisualInfo)
               -> Result<Context, CreationError>
    {
        // creating GL context
        let (context, extra_functions) = unsafe {
            let mut attributes = Vec::new();

            match builder.gl_version {
                GlRequest::Latest => {},
                GlRequest::Specific(Api::OpenGl, (major, minor)) => {
                    attributes.push(ffi::glx_extra::CONTEXT_MAJOR_VERSION_ARB as libc::c_int);
                    attributes.push(major as libc::c_int);
                    attributes.push(ffi::glx_extra::CONTEXT_MINOR_VERSION_ARB as libc::c_int);
                    attributes.push(minor as libc::c_int);
                },
                GlRequest::Specific(_, _) => panic!("Only OpenGL is supported"),
                GlRequest::GlThenGles { opengl_version: (major, minor), .. } => {
                    attributes.push(ffi::glx_extra::CONTEXT_MAJOR_VERSION_ARB as libc::c_int);
                    attributes.push(major as libc::c_int);
                    attributes.push(ffi::glx_extra::CONTEXT_MINOR_VERSION_ARB as libc::c_int);
                    attributes.push(minor as libc::c_int);
                },
            }

            if let Some(core) = builder.gl_core {
                attributes.push(ffi::glx_extra::CONTEXT_PROFILE_MASK_ARB as libc::c_int);
                attributes.push(if core {
                        ffi::glx_extra::CONTEXT_CORE_PROFILE_BIT_ARB
                    } else {
                        ffi::glx_extra::CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB
                    } as libc::c_int
                );
            }

            if builder.gl_debug {
                attributes.push(ffi::glx_extra::CONTEXT_FLAGS_ARB as libc::c_int);
                attributes.push(ffi::glx_extra::CONTEXT_DEBUG_BIT_ARB as libc::c_int);
            }

            attributes.push(0);

            // loading the extra GLX functions
            let extra_functions = ffi::glx_extra::Glx::load_with(|addr| {
                with_c_str(addr, |s| {
                    ffi::glx::GetProcAddress(s as *const u8) as *const libc::c_void
                })
            });

            let share = if let Some(win) = builder.sharing {
                match win.x.context {
                    ::api::x11::Context::Glx(ref c) => c.context,
                    _ => panic!("Cannot share contexts between different APIs")
                }
            } else {
                ptr::null()
            };

            let mut context = if extra_functions.CreateContextAttribsARB.is_loaded() {
                extra_functions.CreateContextAttribsARB(display as *mut ffi::glx_extra::types::Display,
                    fb_config, share, 1, attributes.as_ptr())
            } else {
                ptr::null()
            };

            if context.is_null() {
                context = ffi::glx::CreateContext(display as *mut _, &mut visual_infos, share, 1)
            }

            if context.is_null() {
                return Err(CreationError::OsError(format!("GL context creation failed")));
            }

            (context, extra_functions)
        };

        // vsync
        if builder.vsync {
            unsafe { ffi::glx::MakeCurrent(display as *mut _, window, context) };

            if extra_functions.SwapIntervalEXT.is_loaded() {
                // this should be the most common extension
                unsafe {
                    extra_functions.SwapIntervalEXT(display as *mut _, window, 1);
                }

                // checking that it worked
                if builder.strict {
                    let mut swap = unsafe { mem::uninitialized() };
                    unsafe {
                        ffi::glx::QueryDrawable(display as *mut _, window,
                                                ffi::glx_extra::SWAP_INTERVAL_EXT as i32,
                                                &mut swap);
                    }

                    if swap != 1 {
                        return Err(CreationError::OsError(format!("Couldn't setup vsync: expected \
                                                    interval `1` but got `{}`", swap)));
                    }
                }

            // GLX_MESA_swap_control is not official
            /*} else if extra_functions.SwapIntervalMESA.is_loaded() {
                unsafe {
                    extra_functions.SwapIntervalMESA(1);
                }*/

            } else if extra_functions.SwapIntervalSGI.is_loaded() {
                unsafe {
                    extra_functions.SwapIntervalSGI(1);
                }

            } else if builder.strict {
                return Err(CreationError::OsError(format!("Couldn't find any available vsync extension")));
            }

            unsafe { ffi::glx::MakeCurrent(display as *mut _, 0, ptr::null()) };
        }

        Ok(Context {
            display: display,
            window: window,
            context: context,
        })
    }
}

impl GlContext for Context {
    unsafe fn make_current(&self) {
        let res = ffi::glx::MakeCurrent(self.display as *mut _, self.window, self.context);
        if res == 0 {
            panic!("glx::MakeCurrent failed");
        }
    }

    fn is_current(&self) -> bool {
        unsafe { ffi::glx::GetCurrentContext() == self.context }
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        unsafe {
            ffi::glx::GetProcAddress(addr as *const _) as *const _
        }
    }

    fn swap_buffers(&self) {
        unsafe {
            ffi::glx::SwapBuffers(self.display as *mut _, self.window)
        }
    }

    fn get_api(&self) -> ::Api {
        ::Api::OpenGl
    }

    fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!();
    }
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            // we don't call MakeCurrent(0, 0) because we are not sure that the context
            // is still the current one
            ffi::glx::DestroyContext(self.display as *mut _, self.context);
        }
    }
}
