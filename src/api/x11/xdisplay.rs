use std::ptr;
use std::ffi::CString;

use libc;

use super::ffi;
use api::egl::ffi::egl::Egl;
use api::dlopen;

/// A connection to an X server.
pub struct XConnection {
    pub xlib: ffi::Xlib,
    pub xf86vmode: ffi::Xf86vmode,
    pub xcursor: ffi::Xcursor,
    pub xinput2: ffi::XInput2,
    pub glx: Option<ffi::glx::Glx>,
    pub egl: Option<Egl>,
    pub display: *mut ffi::Display,
}

unsafe impl Send for XConnection {}
unsafe impl Sync for XConnection {}

/// Error returned if this system doesn't have XLib or can't create an X connection.
#[derive(Copy, Clone, Debug)]
pub struct XNotSupported;

impl XConnection {
    pub fn new() -> Result<XConnection, XNotSupported> {
        // opening the libraries
        let xlib = try!(ffi::Xlib::open().map_err(|_| XNotSupported));
        let xcursor = try!(ffi::Xcursor::open().map_err(|_| XNotSupported));
        let xf86vmode = try!(ffi::Xf86vmode::open().map_err(|_| XNotSupported));
        let xinput2 = try!(ffi::XInput2::open().map_err(|_| XNotSupported));

        unsafe extern "C" fn x_error_callback(_: *mut ffi::Display, event: *mut ffi::XErrorEvent)
                                              -> libc::c_int
        {
            println!("[glutin] x error code={} major={} minor={}!", (*event).error_code,
                     (*event).request_code, (*event).minor_code);
            0
        }

        unsafe { (xlib.XInitThreads)() };
        unsafe { (xlib.XSetErrorHandler)(Some(x_error_callback)) };

        // TODO: use something safer than raw "dlopen"
        let glx = {
            let mut libglx = unsafe { dlopen::dlopen(b"libGL.so.1\0".as_ptr() as *const _, dlopen::RTLD_NOW) };
            if libglx.is_null() {
                libglx = unsafe { dlopen::dlopen(b"libGL.so\0".as_ptr() as *const _, dlopen::RTLD_NOW) };
            }

            if libglx.is_null() {
                None
            } else {
                Some(ffi::glx::Glx::load_with(|sym| {
                    let sym = CString::new(sym).unwrap();
                    unsafe { dlopen::dlsym(libglx, sym.as_ptr()) }
                }))
            }
        };

        // TODO: use something safer than raw "dlopen"
        let egl = {
            let libegl = unsafe { dlopen::dlopen(b"libEGL.so\0".as_ptr() as *const _, dlopen::RTLD_NOW) };

            if libegl.is_null() {
                None
            } else {
                Some(Egl::load_with(|sym| {
                    let sym = CString::new(sym).unwrap();
                    unsafe { dlopen::dlsym(libegl, sym.as_ptr()) }
                }))
            }
        };

        // calling XOpenDisplay
        let display = unsafe {
            let display = (xlib.XOpenDisplay)(ptr::null());
            if display.is_null() {
                return Err(XNotSupported);
            }
            display
        };

        Ok(XConnection {
            xlib: xlib,
            xf86vmode: xf86vmode,
            xcursor: xcursor,
            xinput2: xinput2,
            glx: glx,
            egl: egl,
            display: display,
        })
    }
}

impl Drop for XConnection {
    fn drop(&mut self) {
        unsafe { (self.xlib.XCloseDisplay)(self.display) };
    }
}
