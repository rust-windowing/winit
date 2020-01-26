use std::{collections::HashMap, fmt, os::raw::c_int, sync::Arc};

use glutin_x11_sym::{Display, X11_DISPLAY};
use parking_lot::Mutex;
use winit_types::error::Error;

use crate::window::CursorIcon;

use super::ffi;
use super::monitor::MonitorInfoSource;

/// A connection to an X server.
pub struct XConnection {
    pub display: Arc<Display>,
    pub x11_fd: c_int,
    pub cursor_cache: Mutex<HashMap<Option<CursorIcon>, ffi::Cursor>>,
    pub monitor_info_source: MonitorInfoSource,
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
        let x11_fd = unsafe { (syms!(XLIB).XConnectionNumber)(**display) };

        let mut monitor_info_source = MonitorInfoSource::Xlib;

        match (*ffi::XRANDR_2_2_0).as_ref() {
            Ok(_) => {
                let xrandr = syms!(XRANDR_2_2_0);
                let has_xrandr = unsafe {
                    let mut major = 0;
                    let mut minor = 0;
                    (xrandr.XRRQueryVersion)(**display, &mut major, &mut minor)
                };

                match has_xrandr {
                    0 => debug!("[winit] Queried for RANDR version but failed with {:?}, falling back to Xinerama.", has_xrandr),
                    _ => monitor_info_source = MonitorInfoSource::XRandR,
                }
            }
            Err(err) => {
                debug!("[winit] Tried to load RANDR ext symbols but failed with {:?}, falling back to Xinerama.", err);
            }
        }

        if monitor_info_source == MonitorInfoSource::Xlib {
            match (*ffi::XINERAMA).as_ref() {
                Ok(_) => {
                    let xinerama = syms!(XINERAMA);
                    let has_xinerama = unsafe {
                        let mut major = 0;
                        let mut minor = 0;
                        (xinerama.XineramaQueryVersion)(**display, &mut major, &mut minor)
                    };

                    match has_xinerama {
                        0 => debug!("[winit] Queried for Xinerama version but failed with {:?}, falling back to nothing.", has_xinerama),
                        _ => unsafe {
                            match (xinerama.XineramaIsActive)(**display) {
                                ffi::True => monitor_info_source = MonitorInfoSource::Xinerama,
                                is_active => debug!("[winit] Queried Xinerama if it was active and it said {:?}, falling back to nothing.", is_active),
                            }
                        }
                    }
                }
                Err(err) => {
                    debug!("[winit] Tried to load Xinerama ext symbols but failed with {:?}, falling back to nothing.", err);
                }
            }
        }

        Ok(XConnection {
            display,
            x11_fd,
            cursor_cache: Default::default(),
            monitor_info_source,
        })
    }
}

impl fmt::Debug for XConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.display.fmt(f)
    }
}
