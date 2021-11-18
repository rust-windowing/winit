pub use imp::*;

#[cfg(not(feature = "xlib"))]
mod imp {
    use crate::platform::unix::XNotSupported;
    use std::os::raw::c_int;
    use xcb_dl::ffi;

    pub struct Xlib {
        pub dpy: usize,
        pub c: *mut ffi::xcb_connection_t,
        pub default_screen_id: c_int,
    }

    pub fn connect() -> Result<Xlib, XNotSupported> {
        unreachable!();
    }

    pub fn use_xlib() -> bool {
        false
    }
}

#[cfg(feature = "xlib")]
mod imp {
    use crate::platform::unix::XNotSupported;
    use crate::platform_impl::x11::XError;
    use parking_lot::Mutex;
    use std::ffi::CStr;
    use std::mem::{ManuallyDrop, MaybeUninit};
    use std::os::raw::{c_char, c_int};
    use std::ptr;
    use std::sync::{Arc, Weak};
    use x11_dl::xlib_xcb::XEventQueueOwner;
    use xcb_dl::ffi;

    pub struct Xlib {
        pub dpy: usize,
        xlib: Arc<x11_dl::xlib::Xlib>,
        pub c: *mut ffi::xcb_connection_t,
        pub default_screen_id: c_int,
    }

    static LIB: Mutex<Option<ManuallyDrop<Weak<x11_dl::xlib::Xlib>>>> =
        parking_lot::const_mutex(None);

    pub fn connect() -> Result<Xlib, XNotSupported> {
        unsafe {
            let mut lib = LIB.lock();
            let xlib = loop {
                if let Some(lib) = &*lib {
                    if let Some(lib) = lib.upgrade() {
                        break lib;
                    }
                }
                match x11_dl::xlib::Xlib::open() {
                    Ok(l) => {
                        let xlib = Arc::new(l);
                        *lib = Some(ManuallyDrop::new(Arc::downgrade(&xlib)));
                        break xlib;
                    }
                    Err(e) => {
                        return Err(XNotSupported::LibraryOpenError {
                            library: "libX11".to_string(),
                            error: e.to_string(),
                        })
                    }
                }
            };
            let xlib_xcb = match x11_dl::xlib_xcb::Xlib_xcb::open() {
                Ok(l) => l,
                Err(e) => {
                    return Err(XNotSupported::LibraryOpenError {
                        library: "libX11-xcb".to_string(),
                        error: e.to_string(),
                    })
                }
            };
            (xlib.XInitThreads)();
            let dpy = (xlib.XOpenDisplay)(ptr::null());
            if dpy.is_null() {
                return Err(XNotSupported::ConnectFailed {
                    error: "Unknown xlib error".to_string(),
                });
            }
            let default_screen_id = (xlib.XDefaultScreen)(dpy);
            let c = (xlib_xcb.XGetXCBConnection)(dpy) as _;
            (xlib_xcb.XSetEventQueueOwner)(dpy, XEventQueueOwner::XCBOwnsEventQueue);
            (xlib.XSetErrorHandler)(Some(x_error_callback));
            Ok(Xlib {
                dpy: dpy as _,
                xlib,
                c,
                default_screen_id,
            })
        }
    }

    impl Drop for Xlib {
        fn drop(&mut self) {
            unsafe {
                (self.xlib.XCloseDisplay)(self.dpy as _);
            }
        }
    }

    unsafe extern "C" fn x_error_callback(
        display: *mut x11_dl::xlib::Display,
        event: *mut x11_dl::xlib::XErrorEvent,
    ) -> c_int {
        let lib = LIB.lock();
        if let Some(lib) = &*lib {
            if let Some(lib) = lib.upgrade() {
                let error = translate_error(&lib, display, &*event);
                log::error!("X11 error: {:#?}", error);
            }
        }
        0
    }

    unsafe fn translate_error(
        lib: &x11_dl::xlib::Xlib,
        display: *mut x11_dl::xlib::Display,
        event: &x11_dl::xlib::XErrorEvent,
    ) -> XError {
        // `assume_init` is safe here because the array consists of `MaybeUninit` values,
        // which do not require initialization.
        let mut buf: [MaybeUninit<c_char>; 1024] = MaybeUninit::uninit().assume_init();
        (lib.XGetErrorText)(
            display,
            event.error_code as c_int,
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as c_int,
        );
        let description = CStr::from_ptr(buf.as_ptr() as *const c_char).to_string_lossy();
        XError {
            description: description.into_owned(),
            error_code: event.error_code,
            request_code: event.request_code,
            minor_code: event.minor_code,
        }
    }

    pub fn use_xlib() -> bool {
        std::env::var_os("WINIT_DISABLE_XLIB").is_none()
    }
}
