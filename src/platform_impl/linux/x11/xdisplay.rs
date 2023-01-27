use std::{
    collections::{HashMap, VecDeque},
    error::Error,
    fmt,
    mem::ManuallyDrop,
    os::raw::c_int,
    ptr::{self, NonNull},
    sync::Mutex,
};

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::{xproto, Event};
use x11rb::resource_manager;
use x11rb::xcb_ffi::XCBConnection;

use crate::window::CursorIcon;

use super::atoms::*;
use super::ffi;

/// A connection to an X server.
pub(crate) struct XConnection {
    /// Core Xlib shared library.
    pub xlib: ffi::Xlib,

    /// X11 cursor shared library.
    pub xcursor: ffi::Xcursor,

    /// Pointer to the Xlib display.
    pub display: NonNull<ffi::Display>,

    /// A wrapper around the XCB connection.
    ///
    /// We have to wrap the connection in a `ManuallyDrop` because the `XCBConnection` type
    /// needs to be dropped before the Xlib `Display` is dropped.
    pub connection: ManuallyDrop<XCBConnection>,

    /// The X11 XRM database.
    pub database: resource_manager::Database,

    /// The default screen number.
    pub default_screen: usize,

    /// The file descriptor associated with the X11 connection.
    pub x11_fd: c_int,

    /// The atoms used by the program.
    pub(crate) atoms: Atoms,

    /// The latest X11 error used for error handling.
    pub latest_error: Mutex<Option<XError>>,

    /// Cache of X11 cursors.
    pub cursor_cache: Mutex<HashMap<Option<CursorIcon>, xproto::Cursor>>,

    /// The window manager hints that we support.
    pub supported_hints: Mutex<Vec<xproto::Atom>>,

    /// The name of the window manager.
    pub wm_name: Mutex<Option<String>>,

    /// The queue of events that we've received from the X server but haven't acted on yet.
    pub(super) event_queue: Mutex<VecDeque<Event>>,
}

unsafe impl Send for XConnection {}
unsafe impl Sync for XConnection {}

pub type XErrorHandler =
    Option<unsafe extern "C" fn(*mut ffi::Display, *mut ffi::XErrorEvent) -> libc::c_int>;

impl XConnection {
    pub fn new(error_handler: XErrorHandler) -> Result<XConnection, XNotSupported> {
        // opening the libraries
        let xlib = ffi::Xlib::open()?;
        let xcursor = ffi::Xcursor::open()?;
        let xlib_xcb = ffi::Xlib_xcb::open()?;

        unsafe { (xlib.XInitThreads)() };
        unsafe { (xlib.XSetErrorHandler)(error_handler) };

        // calling XOpenDisplay
        let display = unsafe {
            let display = (xlib.XOpenDisplay)(ptr::null());
            match NonNull::new(display) {
                Some(display) => display,
                None => return Err(XNotSupported::XOpenDisplayFailed),
            }
        };

        // Get X11 socket file descriptor
        let fd = unsafe { (xlib.XConnectionNumber)(display.as_ptr()) };

        // Default screen number.
        let default_screen = unsafe { (xlib.XDefaultScreen)(display.as_ptr()) } as usize;

        // Create a new wrapper around the XCB connection.
        let connection = {
            let raw_conn = unsafe { (xlib_xcb.XGetXCBConnection)(display.as_ptr()) };
            debug_assert!(!raw_conn.is_null());

            // Switch the event queue owner so XCB can be used to process events.
            unsafe {
                (xlib_xcb.XSetEventQueueOwner)(
                    display.as_ptr(),
                    ffi::XEventQueueOwner::XCBOwnsEventQueue,
                );
            }

            // Create the x11rb wrapper.
            unsafe { XCBConnection::from_raw_xcb_connection(raw_conn, false) }
                .expect("Failed to create x11rb connection")
        };

        // Prefetch the extensions that we'll use.
        macro_rules! prefetch {
            ($($ext:ident),*) => {
                $(
                    connection.prefetch_extension_information(x11rb::protocol::$ext::X11_EXTENSION_NAME)
                        .expect(concat!("Failed to prefetch ", stringify!($ext)));
                )*
            }
        }

        prefetch!(randr, xinput);

        // Begin loading the atoms.
        let atom_cookie = Atoms::request(&connection).expect("Failed to load atoms");

        // Load the resource manager database.
        let database = resource_manager::new_from_default(&connection)
            .expect("Failed to load resource manager database");

        // Finish loading the atoms.
        let atoms = atom_cookie.reply().expect("Failed to load atoms");

        Ok(XConnection {
            xlib,
            xcursor,
            display,
            connection: ManuallyDrop::new(connection),
            database,
            default_screen,
            x11_fd: fd,
            atoms,
            latest_error: Mutex::new(None),
            cursor_cache: Default::default(),
            supported_hints: Mutex::new(Vec::new()),
            wm_name: Mutex::new(None),
            event_queue: Mutex::new(VecDeque::new()),
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

    /// Get the default screen for this connection.
    #[inline]
    pub fn default_screen(&self) -> &x11rb::protocol::xproto::Screen {
        self.connection
            .setup()
            .roots
            .get(self.default_screen)
            .unwrap()
    }
}

impl fmt::Debug for XConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.display.fmt(f)
    }
}

impl Drop for XConnection {
    #[inline]
    fn drop(&mut self) {
        // Make sure that the XCB connection is dropped before the Xlib connection.
        unsafe {
            ManuallyDrop::drop(&mut self.connection);
            (self.xlib.XCloseDisplay)(self.display.as_ptr());
        }
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

impl Error for XError {}

impl fmt::Display for XError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            formatter,
            "X error: {} (code: {}, request code: {}, minor code: {})",
            self.description, self.error_code, self.request_code, self.minor_code
        )
    }
}

/// Error returned if this system doesn't have XLib or can't create an X connection.
#[derive(Clone, Debug)]
pub enum XNotSupported {
    /// Failed to load one or several shared libraries.
    LibraryOpenError(ffi::OpenError),

    /// Connecting to the X server with `XOpenDisplay` failed.
    XOpenDisplayFailed, // TODO: add better message
}

impl From<ffi::OpenError> for XNotSupported {
    #[inline]
    fn from(err: ffi::OpenError) -> XNotSupported {
        XNotSupported::LibraryOpenError(err)
    }
}

impl XNotSupported {
    fn description(&self) -> &'static str {
        match self {
            XNotSupported::LibraryOpenError(_) => "Failed to load one of xlib's shared libraries",
            XNotSupported::XOpenDisplayFailed => "Failed to open connection to X server",
        }
    }
}

impl Error for XNotSupported {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            XNotSupported::LibraryOpenError(ref err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for XNotSupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        formatter.write_str(self.description())
    }
}
