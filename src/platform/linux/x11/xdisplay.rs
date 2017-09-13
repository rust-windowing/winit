use std::ptr;
use std::fmt;
use std::error::Error;
use std::sync::Mutex;

use libc;

use super::ffi;

/// A connection to an X server.
pub struct XConnection {
    pub xlib: ffi::Xlib,
    pub xrandr: ffi::Xrandr,
    pub xcursor: ffi::Xcursor,
    pub xinput2: ffi::XInput2,
    pub xlib_xcb: ffi::Xlib_xcb,
    pub display: *mut ffi::Display,
    pub latest_error: Mutex<Option<XError>>,
}

unsafe impl Send for XConnection {}
unsafe impl Sync for XConnection {}

pub type XErrorHandler = Option<unsafe extern fn(*mut ffi::Display, *mut ffi::XErrorEvent) -> libc::c_int>;

impl XConnection {
    pub fn new(error_handler: XErrorHandler) -> Result<XConnection, XNotSupported> {
        // opening the libraries
        let xlib = try!(ffi::Xlib::open());
        let xcursor = try!(ffi::Xcursor::open());
        let xrandr = try!(ffi::Xrandr::open());
        let xinput2 = try!(ffi::XInput2::open());
        let xlib_xcb = try!(ffi::Xlib_xcb::open());

        unsafe { (xlib.XInitThreads)() };
        unsafe { (xlib.XSetErrorHandler)(error_handler) };

        // calling XOpenDisplay
        let display = unsafe {
            let display = (xlib.XOpenDisplay)(ptr::null());
            if display.is_null() {
                return Err(XNotSupported::XOpenDisplayFailed);
            }
            display
        };

        Ok(XConnection {
            xlib: xlib,
            xrandr: xrandr,
            xcursor: xcursor,
            xinput2: xinput2,
            xlib_xcb: xlib_xcb,
            display: display,
            latest_error: Mutex::new(None),
        })
    }

    /// Checks whether an error has been triggered by the previous function calls.
    #[inline]
    pub fn check_errors(&self) -> Result<(), XError> {
        let error = self.latest_error.lock().unwrap().take();

        if let Some(error) = error {
            Err(error)
        } else {
            Ok(())
        }
    }

    /// Ignores any previous error.
    #[inline]
    pub fn ignore_error(&self) {
        *self.latest_error.lock().unwrap() = None;
    }
}

impl Drop for XConnection {
    #[inline]
    fn drop(&mut self) {
        unsafe { (self.xlib.XCloseDisplay)(self.display) };
    }
}

/// Error triggered by xlib.
#[derive(Debug, Clone)]
pub struct XError {
    pub description: String,
    pub error_code: u8,
    pub request_code: u8,
    pub minor_code: u8,
}

impl Error for XError {
    #[inline]
    fn description(&self) -> &str {
        &self.description
    }
}

impl fmt::Display for XError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(formatter, "X error: {} (code: {}, request code: {}, minor code: {})",
               self.description, self.error_code, self.request_code, self.minor_code)
    }
}

/// Error returned if this system doesn't have XLib or can't create an X connection.
#[derive(Clone, Debug)]
pub enum XNotSupported {
    /// Failed to load one or several shared libraries.
    LibraryOpenError(ffi::OpenError),
    /// Connecting to the X server with `XOpenDisplay` failed.
    XOpenDisplayFailed,     // TODO: add better message
}

impl From<ffi::OpenError> for XNotSupported {
    #[inline]
    fn from(err: ffi::OpenError) -> XNotSupported {
        XNotSupported::LibraryOpenError(err)
    }
}

impl Error for XNotSupported {
    #[inline]
    fn description(&self) -> &str {
        match *self {
            XNotSupported::LibraryOpenError(_) => "Failed to load one of xlib's shared libraries",
            XNotSupported::XOpenDisplayFailed => "Failed to open connection to X server",
        }
    }

    #[inline]
    fn cause(&self) -> Option<&Error> {
        match *self {
            XNotSupported::LibraryOpenError(ref err) => Some(err),
            _ => None
        }
    }
}

impl fmt::Display for XNotSupported {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        formatter.write_str(self.description())
    }
}
