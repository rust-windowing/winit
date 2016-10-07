//! The purpose of this library is to provide an OpenGL context on as many
//!  platforms as possible.
//!
//! # Building a window
//!
//! There are two ways to create a window:
//!
//!  - Calling `Window::new()`.
//!  - Calling `let builder = WindowBuilder::new()` then `builder.build()`.
//!
//! The first way is the simpliest way and will give you default values.
//!
//! The second way allows you to customize the way your window and GL context
//!  will look and behave.
//!
//! # Features
//!
//! This crate has two Cargo features: `window` and `headless`.
//!
//!  - `window` allows you to create regular windows and enables the `WindowBuilder` object.
//!  - `headless` allows you to do headless rendering, and enables
//!     the `HeadlessRendererBuilder` object.
//!
//! By default only `window` is enabled.

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate shared_library;

extern crate libc;

#[cfg(target_os = "windows")]
extern crate winapi;
#[cfg(target_os = "windows")]
extern crate kernel32;
#[cfg(target_os = "windows")]
extern crate shell32;
#[cfg(target_os = "windows")]
extern crate gdi32;
#[cfg(target_os = "windows")]
extern crate user32;
#[cfg(target_os = "windows")]
extern crate dwmapi;
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[macro_use]
extern crate objc;
#[cfg(target_os = "macos")]
extern crate cgl;
#[cfg(target_os = "macos")]
extern crate cocoa;
#[cfg(target_os = "macos")]
extern crate core_foundation;
#[cfg(target_os = "macos")]
extern crate core_graphics;
#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]
extern crate x11_dl;
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd"))]
#[macro_use(wayland_env,declare_handler)]
extern crate wayland_client;

pub use events::*;
pub use window::{WindowProxy, PollEventsIterator, WaitEventsIterator};
pub use window::{AvailableMonitorsIter, MonitorId, get_available_monitors, get_primary_monitor};
pub use native_monitor::NativeMonitorId;

mod api;
mod platform;
mod events;
mod window;

pub mod os;

/// Represents an OpenGL context and the Window or environment around it.
///
/// # Example
///
/// ```ignore
/// let window = Window::new().unwrap();
///
/// unsafe { window.make_current() };
///
/// loop {
///     for event in window.poll_events() {
///         match(event) {
///             // process events here
///             _ => ()
///         }
///     }
///
///     // draw everything here
///
///     window.swap_buffers();
///     std::old_io::timer::sleep(17);
/// }
/// ```
pub struct Window {
    window: platform::Window,
}

/// Object that allows you to build windows.
#[derive(Clone)]
pub struct WindowBuilder {
    /// The attributes to use to create the window.
    pub window: WindowAttributes,

    /// Platform-specific configuration.
    platform_specific: platform::PlatformSpecificWindowBuilderAttributes,
}

/// Error that can happen while creating a window or a headless renderer.
#[derive(Debug)]
pub enum CreationError {
    OsError(String),
    /// TODO: remove this error
    NotSupported,
}

impl CreationError {
    fn to_string(&self) -> &str {
        match *self {
            CreationError::OsError(ref text) => &text,
            CreationError::NotSupported => "Some of the requested attributes are not supported",
        }
    }
}

impl std::fmt::Display for CreationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        formatter.write_str(self.to_string())
    }
}

impl std::error::Error for CreationError {
    fn description(&self) -> &str {
        self.to_string()
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MouseCursor {
    /// The platform-dependent default cursor.
    Default,
    /// A simple crosshair.
    Crosshair,
    /// A hand (often used to indicate links in web browsers).
    Hand,
    /// Self explanatory.
    Arrow,
    /// Indicates something is to be moved.
    Move,
    /// Indicates text that may be selected or edited.
    Text,
    /// Program busy indicator.
    Wait,
    /// Help indicator (often rendered as a "?")
    Help,
    /// Progress indicator. Shows that processing is being done. But in contrast
    /// with "Wait" the user may still interact with the program. Often rendered
    /// as a spinning beach ball, or an arrow with a watch or hourglass.
    Progress,

    /// Cursor showing that something cannot be done.
    NotAllowed,
    ContextMenu,
    NoneCursor,
    Cell,
    VerticalText,
    Alias,
    Copy,
    NoDrop,
    Grab,
    Grabbing,
    AllScroll,
    ZoomIn,
    ZoomOut,

    /// Indicate that some edge is to be moved. For example, the 'SeResize' cursor
    /// is used when the movement starts from the south-east corner of the box.
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
}

/// Describes how glutin handles the cursor.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CursorState {
    /// Normal cursor behavior.
    Normal,

    /// The cursor will be invisible when over the window.
    Hide,

    /// Grabs the mouse cursor. The cursor's motion will be confined to this
    /// window and the window has exclusive access to further events regarding
    /// the cursor.
    ///
    /// This is useful for first-person cameras for example.
    Grab,
}

/// Attributes to use when creating a window.
#[derive(Clone)]
pub struct WindowAttributes {
    /// The dimensions of the window. If this is `None`, some platform-specific dimensions will be
    /// used.
    ///
    /// The default is `None`.
    pub dimensions: Option<(u32, u32)>,

    /// The minimum dimensions a window can be, If this is `None`, the window will have no minimum dimensions (aside from reserved).
    ///
    /// The default is `None`.
    pub min_dimensions: Option<(u32, u32)>,

    /// The maximum dimensions a window can be, If this is `None`, the maximum will have no maximum or will be set to the primary monitor's dimensions by the platform.
    ///
    /// The default is `None`.
    pub max_dimensions: Option<(u32, u32)>,

    /// If `Some`, the window will be in fullscreen mode with the given monitor.
    ///
    /// The default is `None`.
    pub monitor: Option<platform::MonitorId>,

    /// The title of the window in the title bar.
    ///
    /// The default is `"glutin window"`.
    pub title: String,

    /// Whether the window should be immediately visible upon creation.
    ///
    /// The default is `true`.
    pub visible: bool,

    /// Whether the the window should be transparent. If this is true, writing colors
    /// with alpha values different than `1.0` will produce a transparent window.
    ///
    /// The default is `false`.
    pub transparent: bool,

    /// Whether the window should have borders and bars.
    ///
    /// The default is `true`.
    pub decorations: bool,

    /// [iOS only] Enable multitouch, see [UIView#multipleTouchEnabled]
    /// (https://developer.apple.com/library/ios/documentation/UIKit/Reference/UIView_Class/#//apple_ref/occ/instp/UIView/multipleTouchEnabled)
    pub multitouch: bool,
}

impl Default for WindowAttributes {
    #[inline]
    fn default() -> WindowAttributes {
        WindowAttributes {
            dimensions: None,
            min_dimensions: None,
            max_dimensions: None,
            monitor: None,
            title: "glutin window".to_owned(),
            visible: true,
            transparent: false,
            decorations: true,
            multitouch: false,
        }
    }
}

mod native_monitor {
    /// Native platform identifier for a monitor. Different platforms use fundamentally different types
    /// to represent a monitor ID.
    #[derive(Clone, PartialEq, Eq)]
    pub enum NativeMonitorId {
        /// Cocoa and X11 use a numeric identifier to represent a monitor.
        Numeric(u32),

        /// Win32 uses a Unicode string to represent a monitor.
        Name(String),

        /// Other platforms (Android) don't support monitor identification.
        Unavailable
    }
}
