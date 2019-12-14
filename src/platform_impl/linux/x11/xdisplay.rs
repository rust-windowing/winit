use std::{collections::HashMap, fmt, os::raw::c_int, ptr, sync::Arc};

use glutin_x11_sym::{Display, X11_DISPLAY};
use libc;
use parking_lot::Mutex;
use winit_types::error::Error;
use winit_types::platform::{OsError, XNotSupported};

use crate::window::CursorIcon;

use super::ffi;

/// A connection to an X server.
pub struct XConnection {
    pub display: Arc<Display>,
    pub x11_fd: c_int,
    pub cursor_cache: Mutex<HashMap<Option<CursorIcon>, ffi::Cursor>>,
}

impl XConnection {
    pub fn new() -> Result<XConnection, Error> {
        // opening the libraries
        (*ffi::XLIB)
            .as_ref()
            .map_err(|err| make_oserror!(err.clone().into()))?;
        (*ffi::XCURSOR)
            .as_ref()
            .map_err(|err| make_oserror!(err.clone().into()))?;
        (*ffi::XRANDR_2_2_0)
            .as_ref()
            .map_err(|err| make_oserror!(err.clone().into()))?;
        (*ffi::XINPUT2)
            .as_ref()
            .map_err(|err| make_oserror!(err.clone().into()))?;
        (*ffi::XLIB_XCB)
            .as_ref()
            .map_err(|err| make_oserror!(err.clone().into()))?;

        let display = X11_DISPLAY
            .lock()
            .as_ref()
            .map(Arc::clone)
            .map_err(|err| err.clone())?;

        // Get X11 socket file descriptor
        let fd = unsafe { (syms!(XLIB).XConnectionNumber)(**display) };

        Ok(XConnection {
            display,
            x11_fd: fd,
            cursor_cache: Default::default(),
        })
    }
}

impl fmt::Debug for XConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.display.fmt(f)
    }
}
