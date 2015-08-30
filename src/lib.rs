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

extern crate gl_common;
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
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly"))]
extern crate x11_dl;

pub use events::*;
pub use headless::{HeadlessRendererBuilder, HeadlessContext};
#[cfg(feature = "window")]
pub use window::{WindowBuilder, Window, WindowProxy, PollEventsIterator, WaitEventsIterator};
#[cfg(feature = "window")]
pub use window::{AvailableMonitorsIter, MonitorID, get_available_monitors, get_primary_monitor};
#[cfg(feature = "window")]
pub use native_monitor::NativeMonitorId;

use std::io;
use std::cmp::Ordering;

mod api;
mod platform;
mod events;
mod headless;
#[cfg(feature = "window")]
mod window;

/// Trait that describes objects that have access to an OpenGL context.
pub trait GlContext {
    /// Sets the context as the current context.
    unsafe fn make_current(&self) -> Result<(), ContextError>;

    /// Returns true if this context is the current one in this thread.
    fn is_current(&self) -> bool;

    /// Returns the address of an OpenGL function.
    fn get_proc_address(&self, addr: &str) -> *const libc::c_void;

    /// Swaps the buffers in case of double or triple buffering.
    ///
    /// You should call this function every time you have finished rendering, or the image
    /// may not be displayed on the screen.
    ///
    /// **Warning**: if you enabled vsync, this function will block until the next time the screen
    /// is refreshed. However drivers can choose to override your vsync settings, which means that
    /// you can't know in advance whether `swap_buffers` will block or not.
    fn swap_buffers(&self) -> Result<(), ContextError>;

    /// Returns the OpenGL API being used.
    fn get_api(&self) -> Api;

    /// Returns the pixel format of the main framebuffer of the context.
    fn get_pixel_format(&self) -> PixelFormat;
}

/// Error that can happen while creating a window or a headless renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreationError {
    OsError(String),
    /// TODO: remove this error
    NotSupported,
    RobustnessNotSupported,
    OpenGlVersionNotSupported,
    NoAvailablePixelFormat,
}

impl CreationError {
    fn to_string(&self) -> &str {
        match *self {
            CreationError::OsError(ref text) => &text,
            CreationError::NotSupported => "Some of the requested attributes are not supported",
            CreationError::RobustnessNotSupported => "You requested robustness, but it is \
                                                      not supported.",
            CreationError::OpenGlVersionNotSupported => "The requested OpenGL version is not \
                                                         supported.",
            CreationError::NoAvailablePixelFormat => "Couldn't find any pixel format that matches \
                                                      the criterias.",
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

/// Error that can happen when manipulating an OpenGL context.
#[derive(Debug)]
pub enum ContextError {
    IoError(io::Error),
    ContextLost,
}

impl ContextError {
    fn to_string(&self) -> &str {
        use std::error::Error;
        match *self {
            ContextError::IoError(ref err) => err.description(),
            ContextError::ContextLost => "Context lost"
        }
    }
}

impl std::fmt::Display for ContextError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        formatter.write_str(self.to_string())
    }
}

impl std::error::Error for ContextError {
    fn description(&self) -> &str {
        self.to_string()
    }
}

/// All APIs related to OpenGL that you can possibly get while using glutin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Api {
    /// The classical OpenGL. Available on Windows, Linux, OS/X.
    OpenGl,
    /// OpenGL embedded system. Available on Linux, Android.
    OpenGlEs,
    /// OpenGL for the web. Very similar to OpenGL ES.
    WebGl,
}

/// Describes the requested OpenGL context profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlProfile {
    /// Include all the immediate more functions and definitions.
    Compatibility,
    /// Include all the future-compatible functions and definitions.
    Core,
}

/// Describes the OpenGL API and version that are being requested when a context is created.
#[derive(Debug, Copy, Clone)]
pub enum GlRequest {
    /// Request the latest version of the "best" API of this platform.
    ///
    /// On desktop, will try OpenGL.
    Latest,

    /// Request a specific version of a specific API.
    ///
    /// Example: `GlRequest::Specific(Api::OpenGl, (3, 3))`.
    Specific(Api, (u8, u8)),

    /// If OpenGL is available, create an OpenGL context with the specified `opengl_version`.
    /// Else if OpenGL ES or WebGL is available, create a context with the
    /// specified `opengles_version`.
    GlThenGles {
        /// The version to use for OpenGL.
        opengl_version: (u8, u8),
        /// The version to use for OpenGL ES.
        opengles_version: (u8, u8),
    },
}

impl GlRequest {
    /// Extract the desktop GL version, if any.
    pub fn to_gl_version(&self) -> Option<(u8, u8)> {
        match self {
            &GlRequest::Specific(Api::OpenGl, version) => Some(version),
            &GlRequest::GlThenGles { opengl_version: version, .. } => Some(version),
            _ => None,
        }
    }
}

/// The minimum core profile GL context. Useful for getting the minimum
/// required GL version while still running on OSX, which often forbids
/// the compatibility profile features.
pub static GL_CORE: GlRequest = GlRequest::Specific(Api::OpenGl, (3, 2));

/// Specifies the tolerance of the OpenGL context to faults. If you accept raw OpenGL commands
/// and/or raw shader code from an untrusted source, you should definitely care about this.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Robustness {
    /// Not everything is checked. Your application can crash if you do something wrong with your
    /// shaders.
    NotRobust,

    /// The driver doesn't check anything. This option is very dangerous. Please know what you're
    /// doing before using it. See the `GL_KHR_no_error` extension.
    ///
    /// Since this option is purely an optimisation, no error will be returned if the backend
    /// doesn't support it. Instead it will automatically fall back to `NotRobust`.
    NoError,

    /// Everything is checked to avoid any crash. The driver will attempt to avoid any problem,
    /// but if a problem occurs the behavior is implementation-defined. You are just guaranteed not
    /// to get a crash.
    RobustNoResetNotification,

    /// Same as `RobustNoResetNotification` but the context creation doesn't fail if it's not
    /// supported.
    TryRobustNoResetNotification,

    /// Everything is checked to avoid any crash. If a problem occurs, the context will enter a
    /// "context lost" state. It must then be recreated. For the moment, glutin doesn't provide a
    /// way to recreate a context with the same window :-/
    RobustLoseContextOnReset,

    /// Same as `RobustLoseContextOnReset` but the context creation doesn't fail if it's not
    /// supported.
    TryRobustLoseContextOnReset,
}

#[derive(Debug, Copy, Clone)]
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
#[derive(Debug, Copy, Clone)]
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

/// Describes a possible format. Unused.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct PixelFormat {
    pub hardware_accelerated: bool,
    pub color_bits: u8,
    pub alpha_bits: u8,
    pub depth_bits: u8,
    pub stencil_bits: u8,
    pub stereoscopy: bool,
    pub double_buffer: bool,
    pub multisampling: Option<u16>,
    pub srgb: bool,
}

/// Attributes
// FIXME: remove `pub` (https://github.com/rust-lang/rust/issues/23585)
#[derive(Clone)]
#[doc(hidden)]
pub struct BuilderAttribs<'a> {
    #[allow(dead_code)]
    headless: bool,
    strict: bool,
    sharing: Option<&'a platform::Window>,
    dimensions: Option<(u32, u32)>,
    title: String,
    monitor: Option<platform::MonitorID>,
    gl_version: GlRequest,
    gl_profile: Option<GlProfile>,
    gl_debug: bool,
    gl_robustness: Robustness,
    vsync: bool,
    visible: bool,
    multisampling: Option<u16>,
    depth_bits: Option<u8>,
    stencil_bits: Option<u8>,
    color_bits: Option<u8>,
    alpha_bits: Option<u8>,
    stereoscopy: bool,
    srgb: Option<bool>,
    transparent: bool,
    decorations: bool,
    multitouch: bool
}

impl BuilderAttribs<'static> {
    fn new() -> BuilderAttribs<'static> {
        BuilderAttribs {
            headless: false,
            strict: false,
            sharing: None,
            dimensions: None,
            title: "glutin window".to_string(),
            monitor: None,
            gl_version: GlRequest::Latest,
            gl_profile: None,
            gl_debug: cfg!(debug_assertions),
            gl_robustness: Robustness::NotRobust,
            vsync: false,
            visible: true,
            multisampling: None,
            depth_bits: None,
            stencil_bits: None,
            color_bits: None,
            alpha_bits: None,
            stereoscopy: false,
            srgb: None,
            transparent: false,
            decorations: true,
            multitouch: false
        }
    }
}

impl<'a> BuilderAttribs<'a> {
    #[allow(dead_code)]
    fn extract_non_static(mut self) -> (BuilderAttribs<'static>, Option<&'a platform::Window>) {
        let sharing = self.sharing.take();

        let new_attribs = BuilderAttribs {
            headless: self.headless,
            strict: self.strict,
            sharing: None,
            dimensions: self.dimensions,
            title: self.title,
            monitor: self.monitor,
            gl_version: self.gl_version,
            gl_profile: self.gl_profile,
            gl_debug: self.gl_debug,
            gl_robustness: self.gl_robustness,
            vsync: self.vsync,
            visible: self.visible,
            multisampling: self.multisampling,
            depth_bits: self.depth_bits,
            stencil_bits: self.stencil_bits,
            color_bits: self.color_bits,
            alpha_bits: self.alpha_bits,
            stereoscopy: self.stereoscopy,
            srgb: self.srgb,
            transparent: self.transparent,
            decorations: self.decorations,
            multitouch: self.multitouch
        };

        (new_attribs, sharing)
    }

    fn choose_pixel_format<T, I>(&self, iter: I) -> Result<(T, PixelFormat), CreationError>
                                 where I: IntoIterator<Item=(T, PixelFormat)>, T: Clone
    {
        // filtering formats that don't match the requirements
        let iter = iter.into_iter().filter(|&(_, ref format)| {
            if format.color_bits < self.color_bits.unwrap_or(0) {
                return false;
            }

            if format.alpha_bits < self.alpha_bits.unwrap_or(0) {
                return false;
            }

            if format.depth_bits < self.depth_bits.unwrap_or(0) {
                return false;
            }

            if format.stencil_bits < self.stencil_bits.unwrap_or(0) {
                return false;
            }

            if !format.stereoscopy && self.stereoscopy {
                return false;
            }

            if let Some(req_ms) = self.multisampling {
                match format.multisampling {
                    Some(val) if val >= req_ms => (),
                    _ => return false
                }
            } else {
                if format.multisampling.is_some() {
                    return false;
                }
            }

            if let Some(srgb) = self.srgb {
                if srgb != format.srgb {
                    return false;
                }
            }

            true
        });

        // sorting so that the preferred format comes first
        let mut formats = iter.collect::<Vec<_>>();
        formats.sort_by(|&(_, ref left), &(_, ref right)| {
            // prefer hardware-accelerated formats
            if left.hardware_accelerated && !right.hardware_accelerated {
                return Ordering::Less;
            } else if right.hardware_accelerated && !left.hardware_accelerated {
                return Ordering::Greater;
            }

            // prefer sRGB formats
            if left.srgb && !right.srgb {
                return Ordering::Less;
            } else if right.srgb && !left.srgb {
                return Ordering::Greater;
            }

            // prefer formats with the highest color+alpha bits
            if left.color_bits + left.alpha_bits != right.color_bits + right.alpha_bits {
                return (right.color_bits + right.alpha_bits)
                                                        .cmp(&(left.color_bits + left.alpha_bits));
            }

            // prefer double-buffering formats
            if left.double_buffer && !right.double_buffer {
                return Ordering::Less;
            } else if right.double_buffer && !left.double_buffer {
                return Ordering::Greater;
            }

            // prefer formats with the highest depth bits
            if left.depth_bits != right.depth_bits {
                return (right.depth_bits).cmp(&left.depth_bits);
            }

            // prefer formats with the highest stencil bits
            if left.stencil_bits != right.stencil_bits {
                return (right.stencil_bits).cmp(&left.stencil_bits);
            }

            // prefer formats with multisampling
            if left.multisampling.is_some() && right.multisampling.is_none() {
                return Ordering::Less;
            } else if right.multisampling.is_some() && left.multisampling.is_none() {
                return Ordering::Greater;
            }

            // default
            return Ordering::Equal;
        });

        formats.into_iter().next().ok_or(CreationError::NoAvailablePixelFormat)
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
