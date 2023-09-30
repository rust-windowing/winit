use std::{
    collections::HashMap,
    error::Error,
    fmt, ptr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
};

use crate::window::CursorIcon;

use super::{atoms::Atoms, ffi, monitor::MonitorHandle};
use x11rb::{connection::Connection, protocol::xproto, resource_manager, xcb_ffi::XCBConnection};

/// A connection to an X server.
pub(crate) struct XConnection {
    pub xlib: ffi::Xlib,
    pub xcursor: ffi::Xcursor,

    // TODO(notgull): I'd like to remove this, but apparently Xlib and Xinput2 are tied together
    // for some reason.
    pub xinput2: ffi::XInput2,

    pub display: *mut ffi::Display,

    /// The manager for the XCB connection.
    ///
    /// The `Option` ensures that we can drop it before we close the `Display`.
    xcb: Option<XCBConnection>,

    /// The atoms used by `winit`.
    ///
    /// This is a large structure, so I've elected to Box it to make accessing the fields of
    /// this struct easier. Feel free to unbox it if you like kicking puppies.
    atoms: Box<Atoms>,

    /// The index of the default screen.
    default_screen: usize,

    /// The last timestamp received by this connection.
    timestamp: AtomicU32,

    /// List of monitor handles.
    pub monitor_handles: Mutex<Option<Vec<MonitorHandle>>>,

    /// The resource database.
    database: resource_manager::Database,

    pub latest_error: Mutex<Option<XError>>,
    pub cursor_cache: Mutex<HashMap<Option<CursorIcon>, ffi::Cursor>>,
}

unsafe impl Send for XConnection {}
unsafe impl Sync for XConnection {}

pub type XErrorHandler =
    Option<unsafe extern "C" fn(*mut ffi::Display, *mut ffi::XErrorEvent) -> std::os::raw::c_int>;

impl XConnection {
    pub fn new(error_handler: XErrorHandler) -> Result<XConnection, XNotSupported> {
        // opening the libraries
        let xlib = ffi::Xlib::open()?;
        let xcursor = ffi::Xcursor::open()?;
        let xlib_xcb = ffi::Xlib_xcb::open()?;
        let xinput2 = ffi::XInput2::open()?;

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

        // Open the x11rb XCB connection.
        let xcb = {
            // Get a pointer to the underlying XCB connection
            let xcb_connection =
                unsafe { (xlib_xcb.XGetXCBConnection)(display as *mut ffi::Display) };
            assert!(!xcb_connection.is_null());

            // Wrap the XCB connection in an x11rb XCB connection
            let conn =
                unsafe { XCBConnection::from_raw_xcb_connection(xcb_connection.cast(), false) };

            conn.map_err(|e| XNotSupported::XcbConversionError(Arc::new(WrapConnectError(e))))?
        };

        // Make sure Xlib knows XCB is handling events.
        unsafe {
            (xlib_xcb.XSetEventQueueOwner)(
                display,
                x11_dl::xlib_xcb::XEventQueueOwner::XCBOwnsEventQueue,
            );
        }

        // Get the default screen.
        let default_screen = unsafe { (xlib.XDefaultScreen)(display) } as usize;

        // Fetch the atoms.
        let atoms = Atoms::new(&xcb)
            .map_err(|e| XNotSupported::XcbConversionError(Arc::new(e)))?
            .reply()
            .map_err(|e| XNotSupported::XcbConversionError(Arc::new(e)))?;

        // Load the database.
        let database = resource_manager::new_from_default(&xcb)
            .map_err(|e| XNotSupported::XcbConversionError(Arc::new(e)))?;

        Ok(XConnection {
            xlib,
            xcursor,
            xinput2,
            display,
            xcb: Some(xcb),
            atoms: Box::new(atoms),
            default_screen,
            timestamp: AtomicU32::new(0),
            latest_error: Mutex::new(None),
            monitor_handles: Mutex::new(None),
            database,
            cursor_cache: Default::default(),
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

    /// Get the underlying XCB connection.
    #[inline]
    pub fn xcb_connection(&self) -> &XCBConnection {
        self.xcb
            .as_ref()
            .expect("xcb_connection somehow called after drop?")
    }

    /// Get the list of atoms.
    #[inline]
    pub fn atoms(&self) -> &Atoms {
        &self.atoms
    }

    /// Get the index of the default screen.
    #[inline]
    pub fn default_screen_index(&self) -> usize {
        self.default_screen
    }

    /// Get the default screen.
    #[inline]
    pub fn default_root(&self) -> &xproto::Screen {
        &self.xcb_connection().setup().roots[self.default_screen]
    }

    /// Get the resource database.
    #[inline]
    pub fn database(&self) -> &resource_manager::Database {
        &self.database
    }

    /// Get the latest timestamp.
    #[inline]
    pub fn timestamp(&self) -> u32 {
        self.timestamp.load(Ordering::Relaxed)
    }

    /// Set the last witnessed timestamp.
    #[inline]
    pub fn set_timestamp(&self, timestamp: u32) {
        // Store the timestamp in the slot if it's greater than the last one.
        let mut last_timestamp = self.timestamp.load(Ordering::Relaxed);
        loop {
            let wrapping_sub = |a: xproto::Timestamp, b: xproto::Timestamp| (a as i32) - (b as i32);

            if wrapping_sub(timestamp, last_timestamp) <= 0 {
                break;
            }

            match self.timestamp.compare_exchange(
                last_timestamp,
                timestamp,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => last_timestamp = x,
            }
        }
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
        self.xcb = None;
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
    XOpenDisplayFailed, // TODO: add better message.

    /// We encountered an error while converting the connection to XCB.
    XcbConversionError(Arc<dyn Error + Send + Sync + 'static>),
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
            XNotSupported::XcbConversionError(_) => "Failed to convert Xlib connection to XCB",
        }
    }
}

impl Error for XNotSupported {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            XNotSupported::LibraryOpenError(ref err) => Some(err),
            XNotSupported::XcbConversionError(ref err) => Some(&**err),
            _ => None,
        }
    }
}

impl fmt::Display for XNotSupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        formatter.write_str(self.description())
    }
}

/// A newtype wrapper around a `ConnectError` that can't be accessed by downstream libraries.
///
/// Without this, `x11rb` would become a public dependency.
#[derive(Debug)]
struct WrapConnectError(x11rb::rust_connection::ConnectError);

impl fmt::Display for WrapConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Error for WrapConnectError {
    // We can't implement `source()` here or otherwise risk exposing `x11rb`.
}
