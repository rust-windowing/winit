#![cfg(all(target_os = "linux", feature = "window"))]

use BuilderAttribs;
use ContextError;
use CreationError;
use GlContext;
use GlProfile;
use GlRequest;
use Api;
use PixelFormat;
use Robustness;

use libc;
use std::ffi::{CStr, CString};
use std::{mem, ptr};

use api::x11::ffi;

use platform::Window as PlatformWindow;

pub struct Context {
    glx: ffi::glx::Glx,
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
    pub fn new(glx: ffi::glx::Glx, builder: BuilderAttribs, display: *mut ffi::Display, window: ffi::Window,
               fb_config: ffi::glx::types::GLXFBConfig, mut visual_infos: ffi::glx::types::XVisualInfo)
               -> Result<Context, CreationError>
    {
        let share = if let Some(win) = builder.sharing {
            match win {
                &PlatformWindow::X(ref win) => match win.x.context {
                    ::api::x11::Context::Glx(ref c) => c.context,
                    _ => panic!("Cannot share contexts between different APIs")
                },
                _ => panic!("Cannot use glx on a non-X11 window.")
            }
        } else {
            ptr::null()
        };

        // loading the list of extensions
        let extensions = unsafe {
            let extensions = glx.QueryExtensionsString(display as *mut _, 0);     // FIXME: screen number
            let extensions = CStr::from_ptr(extensions).to_bytes().to_vec();
            String::from_utf8(extensions).unwrap()
        };

        // loading the extra GLX functions
        let extra_functions = ffi::glx_extra::Glx::load_with(|addr| {
            with_c_str(addr, |s| {
                unsafe { glx.GetProcAddress(s as *const u8) as *const _ }
            })
        });

        // creating GL context
        let context = match builder.gl_version {
            GlRequest::Latest => {
                if let Ok(ctxt) = create_context(&glx, &extra_functions, &extensions, (3, 2),
                                                 builder.gl_profile, builder.gl_debug,
                                                 builder.gl_robustness, share,
                                                 display, fb_config, &mut visual_infos)
                {
                    ctxt
                } else if let Ok(ctxt) = create_context(&glx, &extra_functions, &extensions, (3, 1),
                                                        builder.gl_profile, builder.gl_debug,
                                                        builder.gl_robustness, share, display,
                                                        fb_config, &mut visual_infos)
                {
                    ctxt

                } else {
                    try!(create_context(&glx, &extra_functions, &extensions, (1, 0),
                                        builder.gl_profile, builder.gl_debug, builder.gl_robustness,
                                        share, display, fb_config, &mut visual_infos))
                }
            },
            GlRequest::Specific(Api::OpenGl, (major, minor)) => {
                try!(create_context(&glx, &extra_functions, &extensions, (major, minor),
                                    builder.gl_profile, builder.gl_debug, builder.gl_robustness,
                                    share, display, fb_config, &mut visual_infos))
            },
            GlRequest::Specific(_, _) => panic!("Only OpenGL is supported"),
            GlRequest::GlThenGles { opengl_version: (major, minor), .. } => {
                try!(create_context(&glx, &extra_functions, &extensions, (major, minor),
                                    builder.gl_profile, builder.gl_debug, builder.gl_robustness,
                                    share, display, fb_config, &mut visual_infos))
            },
        };

        // vsync
        if builder.vsync {
            unsafe { glx.MakeCurrent(display as *mut _, window, context) };

            if extra_functions.SwapIntervalEXT.is_loaded() {
                // this should be the most common extension
                unsafe {
                    extra_functions.SwapIntervalEXT(display as *mut _, window, 1);
                }

                // checking that it worked
                if builder.strict {
                    let mut swap = unsafe { mem::uninitialized() };
                    unsafe {
                        glx.QueryDrawable(display as *mut _, window,
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

            unsafe { glx.MakeCurrent(display as *mut _, 0, ptr::null()) };
        }

        Ok(Context {
            glx: glx,
            display: display,
            window: window,
            context: context,
        })
    }
}

impl GlContext for Context {
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        // TODO: glutin needs some internal changes for proper error recovery
        let res = self.glx.MakeCurrent(self.display as *mut _, self.window, self.context);
        if res == 0 {
            panic!("glx::MakeCurrent failed");
        }
        Ok(())
    }

    fn is_current(&self) -> bool {
        unsafe { self.glx.GetCurrentContext() == self.context }
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        unsafe {
            self.glx.GetProcAddress(addr as *const _) as *const _
        }
    }

    fn swap_buffers(&self) -> Result<(), ContextError> {
        // TODO: glutin needs some internal changes for proper error recovery
        unsafe { self.glx.SwapBuffers(self.display as *mut _, self.window); }
        Ok(())
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
            if self.is_current() {
                self.glx.MakeCurrent(self.display as *mut _, 0, ptr::null_mut());
            }

            self.glx.DestroyContext(self.display as *mut _, self.context);
        }
    }
}

fn create_context(glx: &ffi::glx::Glx, extra_functions: &ffi::glx_extra::Glx, extensions: &str,
                  version: (u8, u8), profile: Option<GlProfile>, debug: bool,
                  robustness: Robustness, share: ffi::GLXContext, display: *mut ffi::Display,
                  fb_config: ffi::glx::types::GLXFBConfig,
                  visual_infos: &mut ffi::glx::types::XVisualInfo)
                  -> Result<ffi::GLXContext, CreationError>
{
    unsafe {
        let context = if extra_functions.CreateContextAttribsARB.is_loaded() {
            let mut attributes = Vec::with_capacity(9);

            attributes.push(ffi::glx_extra::CONTEXT_MAJOR_VERSION_ARB as libc::c_int);
            attributes.push(version.0 as libc::c_int);
            attributes.push(ffi::glx_extra::CONTEXT_MINOR_VERSION_ARB as libc::c_int);
            attributes.push(version.1 as libc::c_int);

            if let Some(profile) = profile {
                let flag = match profile {
                    GlProfile::Compatibility =>
                        ffi::glx_extra::CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB,
                    GlProfile::Core =>
                        ffi::glx_extra::CONTEXT_CORE_PROFILE_BIT_ARB,
                };

                attributes.push(ffi::glx_extra::CONTEXT_PROFILE_MASK_ARB as libc::c_int);
                attributes.push(flag as libc::c_int);
            }

            let flags = {
                let mut flags = 0;

                // robustness
                if extensions.split(' ').find(|&i| i == "GLX_ARB_create_context_robustness").is_some() {
                    match robustness {
                        Robustness::RobustNoResetNotification | Robustness::TryRobustNoResetNotification => {
                            attributes.push(ffi::glx_extra::CONTEXT_RESET_NOTIFICATION_STRATEGY_ARB as libc::c_int);
                            attributes.push(ffi::glx_extra::NO_RESET_NOTIFICATION_ARB as libc::c_int);
                            flags = flags | ffi::glx_extra::CONTEXT_ROBUST_ACCESS_BIT_ARB as libc::c_int;
                        },
                        Robustness::RobustLoseContextOnReset | Robustness::TryRobustLoseContextOnReset => {
                            attributes.push(ffi::glx_extra::CONTEXT_RESET_NOTIFICATION_STRATEGY_ARB as libc::c_int);
                            attributes.push(ffi::glx_extra::LOSE_CONTEXT_ON_RESET_ARB as libc::c_int);
                            flags = flags | ffi::glx_extra::CONTEXT_ROBUST_ACCESS_BIT_ARB as libc::c_int;
                        },
                        Robustness::NotRobust => (),
                        Robustness::NoError => (),
                    }
                } else {
                    match robustness {
                        Robustness::RobustNoResetNotification | Robustness::RobustLoseContextOnReset => {
                            return Err(CreationError::NotSupported);
                        },
                        _ => ()
                    }
                }

                if debug {
                    flags = flags | ffi::glx_extra::CONTEXT_DEBUG_BIT_ARB as libc::c_int;
                }

                flags
            };

            attributes.push(ffi::glx_extra::CONTEXT_FLAGS_ARB as libc::c_int);
            attributes.push(flags);

            attributes.push(0);

            extra_functions.CreateContextAttribsARB(display as *mut _, fb_config, share, 1,
                                                    attributes.as_ptr())

        } else {
            glx.CreateContext(display as *mut _, visual_infos, share, 1)
        };

        if context.is_null() {
            return Err(CreationError::OsError(format!("GL context creation failed")));
        }

        Ok(context)
    }
}
