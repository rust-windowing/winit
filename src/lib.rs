#![feature(unsafe_destructor,core,std_misc)]
#![unstable]

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

extern crate gl_common;
extern crate libc;

#[cfg(target_os = "windows")]
extern crate winapi;
#[cfg(target_os = "windows")]
extern crate "kernel32-sys" as kernel32;
#[cfg(target_os = "windows")]
extern crate "gdi32-sys" as gdi32;
#[cfg(target_os = "windows")]
extern crate "user32-sys" as user32;
#[cfg(target_os = "macos")]
extern crate cocoa;
#[cfg(target_os = "macos")]
extern crate core_foundation;
#[cfg(target_os = "macos")]
extern crate core_graphics;

pub use events::*;
#[cfg(feature = "headless")]
pub use headless::{HeadlessRendererBuilder, HeadlessContext};
#[cfg(feature = "window")]
pub use window::{WindowBuilder, Window, WindowProxy, PollEventsIterator, WaitEventsIterator};
#[cfg(feature = "window")]
pub use window::{AvailableMonitorsIter, MonitorID, get_available_monitors, get_primary_monitor};

#[cfg(all(not(target_os = "windows"), not(target_os = "linux"), not(target_os = "macos"), not(target_os = "android")))]
use this_platform_is_not_supported;

#[cfg(target_os = "windows")]
#[path="win32/mod.rs"]
mod winimpl;
#[cfg(target_os = "linux")]
#[path="x11/mod.rs"]
mod winimpl;
#[cfg(target_os = "macos")]
#[path="cocoa/mod.rs"]
mod winimpl;
#[cfg(target_os = "android")]
#[path="android/mod.rs"]
mod winimpl;

mod events;
#[cfg(feature = "headless")]
mod headless;
#[cfg(feature = "window")]
mod window;

/// Error that can happen while creating a window or a headless renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreationError {
    OsError(String),
    NotSupported,
}

impl CreationError {
    fn to_string(&self) -> &str {
        match *self {
            CreationError::OsError(ref text) => text.as_slice(),
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

#[derive(Debug, Copy)]
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

/// Describes a possible format. Unused.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct PixelFormat {
    pub hardware_accelerated: bool,
    pub red_bits: u8,
    pub green_bits: u8,
    pub blue_bits: u8,
    pub alpha_bits: u8,
    pub depth_bits: u8,
    pub stencil_bits: u8,
    pub stereoscopy: bool,
    pub double_buffer: bool,
    pub multisampling: Option<u16>,
    pub srgb: bool,
}

/// Attributes
struct BuilderAttribs<'a> {
    #[allow(dead_code)]
    headless: bool,
    strict: bool,
    sharing: Option<&'a winimpl::Window>,
    dimensions: Option<(u32, u32)>,
    title: String,
    monitor: Option<winimpl::MonitorID>,
    gl_version: GlRequest,
    gl_debug: bool,
    vsync: bool,
    visible: bool,
    multisampling: Option<u16>,
    depth_bits: Option<u8>,
    stencil_bits: Option<u8>,
    color_bits: Option<u8>,
    alpha_bits: Option<u8>,
    stereoscopy: bool,
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
            gl_debug: cfg!(ndebug),
            vsync: false,
            visible: true,
            multisampling: None,
            depth_bits: None,
            stencil_bits: None,
            color_bits: None,
            alpha_bits: None,
            stereoscopy: false,
        }
    }
}

impl<'a> BuilderAttribs<'a> {
    fn extract_non_static(mut self) -> (BuilderAttribs<'static>, Option<&'a winimpl::Window>) {
        let sharing = self.sharing.take();

        let new_attribs = BuilderAttribs {
            headless: self.headless,
            strict: self.strict,
            sharing: None,
            dimensions: self.dimensions,
            title: self.title,
            monitor: self.monitor,
            gl_version: self.gl_version,
            gl_debug: self.gl_debug,
            vsync: self.vsync,
            visible: self.visible,
            multisampling: self.multisampling,
            depth_bits: self.depth_bits,
            stencil_bits: self.stencil_bits,
            color_bits: self.color_bits,
            alpha_bits: self.alpha_bits,
            stereoscopy: self.stereoscopy,
        };

        (new_attribs, sharing)
    }

    fn choose_pixel_format<T, I>(&self, iter: I) -> (T, PixelFormat)
                                 where I: Iterator<Item=(T, PixelFormat)>, T: Clone
    {
        let mut current_result = None;
        let mut current_software_result = None;

        // TODO: do this more properly
        for (id, format) in iter {
            if format.red_bits + format.green_bits + format.blue_bits < self.color_bits.unwrap_or(0) {
                continue;
            }

            if format.alpha_bits < self.alpha_bits.unwrap_or(0) {
                continue;
            }

            if format.depth_bits < self.depth_bits.unwrap_or(0) {
                continue;
            }

            if format.stencil_bits < self.stencil_bits.unwrap_or(0) {
                continue;
            }

            if !format.stereoscopy && self.stereoscopy {
                continue;
            }

            if self.multisampling.is_some() && format.multisampling.is_none() {
                continue;
            }

            current_software_result = Some((id.clone(), format.clone()));
            if format.hardware_accelerated {
                current_result = Some((id, format));
            }
        }

        current_result.or(current_software_result)
                      .expect("Could not find compliant pixel format")
    }
}
