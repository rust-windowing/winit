use std::sync::Arc;
use std::{collections::HashMap, fmt, mem, os::raw::c_int, ptr};

use parking_lot::Mutex;

use crate::window::CursorIcon;

use super::ffi;
use crate::platform_impl::x11::xlib::Xlib;
use crate::platform_impl::x11::MonitorHandle;
use thiserror::Error;
use xcb_dl::{Xcb, XcbRandr, XcbRender, XcbXfixes, XcbXinput, XcbXkb};
use xcb_dl_util::cursor::XcbCursorContext;
use xcb_dl_util::error::{XcbError, XcbErrorParser};

/// A connection to an X server.
pub struct XConnection {
    pub c: *mut ffi::xcb_connection_t,
    pub fd: c_int,

    pub atom_cache: Mutex<HashMap<String, ffi::xcb_atom_t>>,

    pub default_screen_id: usize,
    pub screens: Vec<Arc<Screen>>,

    pub errors: XcbErrorParser,

    pub xcb: Box<Xcb>,

    pub xkb: Box<XcbXkb>,
    pub xkb_first_event: u8,

    pub xfixes: Box<XcbXfixes>,
    pub xfixes_first_event: u8,

    pub xinput: Box<XcbXinput>,
    pub xinput_extension: u8,

    pub render: Box<XcbRender>,

    pub randr: Box<XcbRandr>,
    pub randr_version: (u32, u32),
    pub randr_first_event: u8,

    pub cursors: XcbCursorContext,
    pub cursor_cache: Mutex<HashMap<Option<CursorIcon>, ffi::xcb_cursor_t>>,

    pub monitors: Mutex<Option<Vec<MonitorHandle>>>,

    pub xlib: Option<Xlib>,
}

unsafe impl Send for XConnection {}
unsafe impl Sync for XConnection {}

impl XConnection {
    pub fn new() -> Result<XConnection, XNotSupported> {
        unsafe { Self::new_unsafe() }
    }

    unsafe fn new_unsafe() -> Result<XConnection, XNotSupported> {
        macro_rules! load {
            ($id:ident, $name:expr) => {
                match $id::load_loose() {
                    Ok(l) => Box::new(l),
                    Err(e) => {
                        return Err(XNotSupported::LibraryOpenError {
                            library: $name.to_string(),
                            error: e.to_string(),
                        })
                    }
                }
            };
        }

        let xcb = load!(Xcb, "libxcb");
        let xfixes = load!(XcbXfixes, "libxcb_xfixes");
        let xinput = load!(XcbXinput, "libxcb_xinput");
        let render = load!(XcbRender, "libxcb_render");
        let xkb = load!(XcbXkb, "libxcb_xkb");
        let randr = load!(XcbRandr, "libxcb_randr");

        let (c, default_screen_id, xlib) = if super::xlib::use_xlib() {
            let xlib = super::xlib::connect()?;
            (xlib.c, xlib.default_screen_id, Some(xlib))
        } else {
            let mut default_screen_id = 0;
            let c = xcb.xcb_connect(ptr::null(), &mut default_screen_id);
            (c, default_screen_id, None)
        };

        let errors = XcbErrorParser::new(&xcb, c);

        if let Err(e) = errors.check_connection(&xcb) {
            return Err(XNotSupported::ConnectFailed {
                error: e.to_string(),
            });
        }

        let close = if xlib.is_some() {
            None
        } else {
            Some(CloseConnection { c, xcb: &xcb })
        };

        xcb_dl_util::log::log_connection(log::Level::Trace, &xcb, c);

        macro_rules! check_ext {
            ($ext:expr, $name:expr) => {{
                let data = xcb.xcb_get_extension_data(c, $ext);
                if data.is_null() || (*data).present == 0 {
                    return Err(XNotSupported::MissingExtension {
                        extension: $name.to_string(),
                    });
                }
                data
            }};
        }

        let xinput_data = check_ext!(xinput.xcb_input_id(), ffi::XCB_INPUT_NAME_STR);
        let xfixes_data = check_ext!(xfixes.xcb_xfixes_id(), ffi::XCB_XFIXES_NAME_STR);
        let _render_data = check_ext!(render.xcb_render_id(), ffi::XCB_RENDER_NAME_STR);
        let xkb_data = check_ext!(xkb.xcb_xkb_id(), ffi::XCB_XKB_NAME_STR);
        let randr_data = check_ext!(randr.xcb_randr_id(), ffi::XCB_RANDR_NAME_STR);

        macro_rules! enable_extension {
            ($so:expr, $query:ident, $reply:ident, $major:expr, $minor:expr, $name:expr) => {
                enable_extension!(
                    $so,
                    $query,
                    $reply,
                    major_version,
                    minor_version,
                    $major,
                    $minor,
                    $name
                )
            };
            ($so:expr, $query:ident, $reply:ident, $server_major:ident, $server_minor:ident, $major:expr, $minor:expr, $name:expr) => {{
                let mut err = ptr::null_mut();
                let res = $so.$reply(c, $so.$query(c, $major, $minor), &mut err);
                let res = match errors.check(&xcb, res, err) {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("Could not enable `{}` extension: {}", $name, e);
                        return Err(XNotSupported::MissingExtension {
                            extension: $name.to_string(),
                        });
                    }
                };
                let version = (res.$server_major, res.$server_major);
                if version < ($major, $minor) {
                    log::warn!(
                        "winit uses the `{}` extension in version {:?} but the X server only \
                        provides version {:?}, some features might be unavailable.",
                        $name,
                        ($major, $minor),
                        version
                    );
                }
                version
            }};
        }

        enable_extension!(
            xfixes,
            xcb_xfixes_query_version,
            xcb_xfixes_query_version_reply,
            1,
            0,
            ffi::XCB_XFIXES_NAME_STR
        );
        enable_extension!(
            xinput,
            xcb_input_xi_query_version,
            xcb_input_xi_query_version_reply,
            2,
            2,
            ffi::XCB_INPUT_NAME_STR
        );
        enable_extension!(
            render,
            xcb_render_query_version,
            xcb_render_query_version_reply,
            0,
            8,
            ffi::XCB_RENDER_NAME_STR
        );
        enable_extension!(
            xkb,
            xcb_xkb_use_extension,
            xcb_xkb_use_extension_reply,
            server_major,
            server_minor,
            1,
            0,
            ffi::XCB_XKB_NAME_STR
        );
        let randr_version = enable_extension!(
            randr,
            xcb_randr_query_version,
            xcb_randr_query_version_reply,
            1,
            3,
            ffi::XCB_RANDR_NAME_STR
        );

        let cursors = XcbCursorContext::new(&xcb, &render, c);

        let fd = xcb.xcb_get_file_descriptor(c);

        let screens = {
            let mut screen_iter = xcb.xcb_setup_roots_iterator(xcb.xcb_get_setup(c));
            let mut screens = vec![];
            let mut i = 0;
            while screen_iter.rem > 0 {
                let screen = &*screen_iter.data;
                screens.push(Arc::new(Screen {
                    screen_id: i,
                    root: screen.root,
                    root_depth: screen.root_depth,
                    supported_hints: Default::default(),
                    wm_name: Default::default(),
                }));
                xcb.xcb_screen_next(&mut screen_iter);
                i += 1;
            }
            screens
        };
        let default_screen_id = default_screen_id.max(0) as usize;
        if default_screen_id > screens.len() {
            return Err(XNotSupported::ConnectFailed {
                error: format!("Default screen id out of bounds"),
            });
        }

        for screen in &screens {
            let screen = &*screen;
            let mask = ffi::XCB_RANDR_NOTIFY_MASK_CRTC_CHANGE
                | ffi::XCB_RANDR_NOTIFY_MASK_OUTPUT_PROPERTY
                | ffi::XCB_RANDR_NOTIFY_MASK_SCREEN_CHANGE;
            let cookie = randr.xcb_randr_select_input_checked(c, screen.root, mask as _);
            if let Err(e) = errors.check_cookie(&xcb, cookie) {
                return Err(XNotSupported::ConnectFailed {
                    error: format!("Cannot listen for RandR events: {}", e),
                });
            }
        }

        mem::forget(close);

        Ok(XConnection {
            atom_cache: Default::default(),
            c,
            fd,
            default_screen_id,
            screens,
            cursors,
            errors,
            xcb,
            xkb,
            xkb_first_event: (*xkb_data).first_event,
            xfixes,
            xfixes_first_event: (*xfixes_data).first_event,
            xinput,
            xinput_extension: (*xinput_data).major_opcode,
            render,
            randr,
            randr_version,
            randr_first_event: (*randr_data).first_event,
            cursor_cache: Default::default(),
            monitors: Default::default(),
            xlib,
        })
    }
}

impl fmt::Debug for XConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XConnection").finish_non_exhaustive()
    }
}

impl Drop for XConnection {
    #[inline]
    fn drop(&mut self) {
        if self.xlib.is_none() {
            unsafe {
                self.xcb.xcb_disconnect(self.c);
            }
        }
    }
}

#[derive(Debug)]
pub struct Screen {
    pub screen_id: usize,
    pub root: ffi::xcb_window_t,
    pub root_depth: u8,
    pub supported_hints: Mutex<Vec<ffi::xcb_atom_t>>,
    pub wm_name: Mutex<Option<String>>,
}

impl Screen {
    pub fn hint_is_supported(&self, hint: ffi::xcb_atom_t) -> bool {
        self.supported_hints.lock().contains(&hint)
    }

    pub fn wm_name_is_one_of(&self, names: &[&str]) -> bool {
        if let Some(ref name) = *self.wm_name.lock() {
            names.contains(&name.as_str())
        } else {
            false
        }
    }
}

unsafe impl Sync for Screen {}
unsafe impl Send for Screen {}

/// Error triggered by xcb.
#[derive(Clone, Debug, Error)]
#[error("X error: {description} (code: {error_code}, request code: {request_code}, minor code: {minor_code})")]
pub struct XError {
    pub description: String,
    pub error_code: u8,
    pub request_code: u8,
    pub minor_code: u8,
}

impl From<XcbError> for XError {
    fn from(e: XcbError) -> Self {
        Self {
            description: e.to_string(),
            error_code: e.error_code,
            request_code: e.major,
            minor_code: e.minor as _,
        }
    }
}

/// Error returned if this system doesn't have libxcb or can't create an X connection.
#[derive(Clone, Debug, Error)]
#[non_exhaustive]
pub enum XNotSupported {
    /// Failed to load one or several shared libraries.
    #[error("Could not open {library}: {error}")]
    LibraryOpenError { library: String, error: String },
    /// A required extension is not available.
    #[error("The required extension {extension} is not available")]
    MissingExtension { extension: String },
    /// Connecting to the X server with `xcb_connect` failed.
    #[error("Cannot connect to the X server: {error}")]
    ConnectFailed { error: String },
    /// The X server has no attached screens.
    #[error("The X server has no attached screens")]
    NoScreens,
}

struct CloseConnection<'a> {
    c: *mut ffi::xcb_connection_t,
    xcb: &'a Xcb,
}

impl<'a> Drop for CloseConnection<'a> {
    fn drop(&mut self) {
        unsafe {
            self.xcb.xcb_disconnect(self.c);
        }
    }
}
