#![feature(unsafe_destructor)]
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
#[cfg(target_os = "macos")]
extern crate cocoa;
#[cfg(target_os = "macos")]
extern crate core_foundation;
#[cfg(target_os = "macos")]
extern crate core_graphics;

pub use events::*;

use std::default::Default;
use std::collections::ring_buf::IntoIter as RingBufIter;

#[cfg(all(not(target_os = "windows"), not(target_os = "linux"), not(target_os = "macos"), not(target_os = "android")))]
use this_platform_is_not_supported;

#[cfg(target_os = "windows")]
#[path="win32/mod.rs"]
mod winimpl;
#[cfg(target_os = "linux")]
#[path="x11/mod.rs"]
mod winimpl;
#[cfg(target_os = "macos")]
#[path="osx/mod.rs"]
mod winimpl;
#[cfg(target_os = "android")]
#[path="android/mod.rs"]
mod winimpl;

mod events;

/// Identifier for a monitor.
#[cfg(feature = "window")]
pub struct MonitorID(winimpl::MonitorID);

/// Error that can happen while creating a window or a headless renderer.
#[derive(Clone, Show, PartialEq, Eq)]
pub enum CreationError {
    OsError(String),
    NotSupported,
}

impl std::error::Error for CreationError {
    fn description(&self) -> &str {
        match self {
            &CreationError::OsError(ref text) => text.as_slice(),
            &CreationError::NotSupported => "Some of the requested attributes are not supported",
        }
    }
}

/// All APIs related to OpenGL that you can possibly get while using glutin.
#[derive(Show, Clone, Copy, PartialEq, Eq)]
pub enum Api {
    /// The classical OpenGL. Available on Windows, Linux, OS/X.
    OpenGl,
    /// OpenGL embedded system. Available on Linux, Android.
    OpenGlEs,
}

#[derive(Show, Copy)]
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

/// Object that allows you to build windows.
#[cfg(feature = "window")]
pub struct WindowBuilder<'a> {
    attribs: BuilderAttribs<'a>
}

/// Attributes
struct BuilderAttribs<'a> {
    headless: bool,
    strict: bool,
    sharing: Option<&'a winimpl::Window>,
    dimensions: Option<(u32, u32)>,
    title: String,
    monitor: Option<winimpl::MonitorID>,
    gl_version: Option<(u32, u32)>,
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
            gl_version: None,
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

#[cfg(feature = "window")]
impl<'a> WindowBuilder<'a> {
    /// Initializes a new `WindowBuilder` with default values.
    pub fn new() -> WindowBuilder<'a> {
        WindowBuilder {
            attribs: BuilderAttribs::new(),
        }
    }

    /// Requests the window to be of specific dimensions.
    ///
    /// Width and height are in pixels.
    pub fn with_dimensions(mut self, width: u32, height: u32) -> WindowBuilder<'a> {
        self.attribs.dimensions = Some((width, height));
        self
    }

    /// Requests a specific title for the window.
    pub fn with_title(mut self, title: String) -> WindowBuilder<'a> {
        self.attribs.title = title;
        self
    }

    /// Requests fullscreen mode.
    ///
    /// If you don't specify dimensions for the window, it will match the monitor's.
    pub fn with_fullscreen(mut self, monitor: MonitorID) -> WindowBuilder<'a> {
        let MonitorID(monitor) = monitor;
        self.attribs.monitor = Some(monitor);
        self
    }

    /// The created window will share all its OpenGL objects with the window in the parameter.
    ///
    /// There are some exceptions, like FBOs or VAOs. See the OpenGL documentation.
    pub fn with_shared_lists(mut self, other: &'a Window) -> WindowBuilder<'a> {
        self.attribs.sharing = Some(&other.window);
        self
    }

    /// Requests to use a specific OpenGL version.
    ///
    /// Version is a (major, minor) pair. For example to request OpenGL 3.3
    ///  you would pass `(3, 3)`.
    pub fn with_gl_version(mut self, version: (u32, u32)) -> WindowBuilder<'a> {
        self.attribs.gl_version = Some(version);
        self
    }

    /// Sets the *debug* flag for the OpenGL context.
    ///
    /// The default value for this flag is `cfg!(ndebug)`, which means that it's enabled
    /// when you run `cargo build` and disabled when you run `cargo build --release`.
    pub fn with_gl_debug_flag(mut self, flag: bool) -> WindowBuilder<'a> {
        self.attribs.gl_debug = flag;
        self
    }

    /// Requests that the window has vsync enabled.
    pub fn with_vsync(mut self) -> WindowBuilder<'a> {
        self.attribs.vsync = true;
        self
    }

    /// Sets whether the window will be initially hidden or visible.
    pub fn with_visibility(mut self, visible: bool) -> WindowBuilder<'a> {
        self.attribs.visible = visible;
        self
    }

    /// Sets the multisampling level to request.
    ///
    /// # Panic
    ///
    /// Will panic if `samples` is not a power of two.
    pub fn with_multisampling(mut self, samples: u16) -> WindowBuilder<'a> {
        use std::num::UnsignedInt;
        assert!(samples.is_power_of_two());
        self.attribs.multisampling = Some(samples);
        self
    }

    /// Sets the number of bits in the depth buffer.
    pub fn with_depth_buffer(mut self, bits: u8) -> WindowBuilder<'a> {
        self.attribs.depth_bits = Some(bits);
        self
    }

    /// Sets the number of bits in the stencil buffer.
    pub fn with_stencil_buffer(mut self, bits: u8) -> WindowBuilder<'a> {
        self.attribs.stencil_bits = Some(bits);
        self
    }

    /// Sets the number of bits in the color buffer.
    pub fn with_pixel_format(mut self, color_bits: u8, alpha_bits: u8) -> WindowBuilder<'a> {
        self.attribs.color_bits = Some(color_bits);
        self.attribs.alpha_bits = Some(alpha_bits);
        self
    }

    /// Request the backend to be stereoscopic.
    pub fn with_stereoscopy(mut self) -> WindowBuilder<'a> {
        self.attribs.stereoscopy = true;
        self
    }

    /// Builds the window.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    pub fn build(mut self) -> Result<Window, CreationError> {
        // resizing the window to the dimensions of the monitor when fullscreen
        if self.attribs.dimensions.is_none() && self.attribs.monitor.is_some() {
            self.attribs.dimensions = Some(self.attribs.monitor.as_ref().unwrap().get_dimensions())
        }

        // default dimensions
        if self.attribs.dimensions.is_none() {
            self.attribs.dimensions = Some((1024, 768));
        }

        // building
        winimpl::Window::new(self.attribs).map(|w| Window { window: w })
    }

    /// Builds the window.
    ///
    /// The context is build in a *strict* way. That means that if the backend couldn't give
    /// you what you requested, an `Err` will be returned.
    pub fn build_strict(mut self) -> Result<Window, CreationError> {
        self.attribs.strict = true;
        self.build()
    }
}

/// Object that allows you to build headless contexts.
#[cfg(feature = "headless")]
pub struct HeadlessRendererBuilder {
    attribs: BuilderAttribs<'static>,
}

#[cfg(feature = "headless")]
impl HeadlessRendererBuilder {
    /// Initializes a new `HeadlessRendererBuilder` with default values.
    pub fn new(width: u32, height: u32) -> HeadlessRendererBuilder {
        HeadlessRendererBuilder {
            attribs: BuilderAttribs {
                headless: true,
                dimensions: Some((width, height)),
                .. BuilderAttribs::new()
            },
        }
    }

    /// Requests to use a specific OpenGL version.
    ///
    /// Version is a (major, minor) pair. For example to request OpenGL 3.3
    ///  you would pass `(3, 3)`.
    pub fn with_gl_version(mut self, version: (u32, u32)) -> HeadlessRendererBuilder {
        self.attribs.gl_version = Some(version);
        self
    }

    /// Sets the *debug* flag for the OpenGL context.
    ///
    /// The default value for this flag is `cfg!(ndebug)`, which means that it's enabled
    /// when you run `cargo build` and disabled when you run `cargo build --release`.
    pub fn with_gl_debug_flag(mut self, flag: bool) -> HeadlessRendererBuilder {
        self.attribs.gl_debug = flag;
        self
    }

    /// Builds the headless context.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    pub fn build(self) -> Result<HeadlessContext, CreationError> {
        winimpl::HeadlessContext::new(self.attribs).map(|w| HeadlessContext { context: w })
    }

    /// Builds the headless context.
    ///
    /// The context is build in a *strict* way. That means that if the backend couldn't give
    /// you what you requested, an `Err` will be returned.
    pub fn build_strict(mut self) -> Result<HeadlessContext, CreationError> {
        self.attribs.strict = true;
        self.build()
    }
}

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
///             // process events here
///             _ => ()
///         }
///     }
///
///     // draw everything here
///
///     window.swap_buffers();
///     std::io::timer::sleep(17);
/// }
/// ```
#[cfg(feature = "window")]
pub struct Window {
    window: winimpl::Window,
}

#[cfg(feature = "window")]
impl Default for Window {
    fn default() -> Window {
        Window::new().unwrap()
    }
}

#[cfg(feature = "window")]
impl Window {
    /// Creates a new OpenGL context, and a Window for platforms where this is appropriate.
    ///
    /// This function is equivalent to `WindowBuilder::new().build()`.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    #[inline]
    #[cfg(feature = "window")]
    pub fn new() -> Result<Window, CreationError> {
        let builder = WindowBuilder::new();
        builder.build()
    }

    /// Returns true if the window has previously been closed by the user.
    #[inline]
    pub fn is_closed(&self) -> bool {
        self.window.is_closed()
    }

    /// Returns true if the window has previously been closed by the user.
    #[inline]
    #[deprecated = "Use is_closed instead"]
    pub fn should_close(&self) -> bool {
        self.is_closed()
    }

    /// Modifies the title of the window.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_title(&self, title: &str) {
        self.window.set_title(title)
    }

    /// Shows the window if it was hidden.
    ///
    /// ## Platform-specific
    ///
    /// - Has no effect on Android
    ///
    #[inline]
    pub fn show(&self) {
        self.window.show()
    }

    /// Hides the window if it was visible.
    ///
    /// ## Platform-specific
    ///
    /// - Has no effect on Android
    ///
    #[inline]
    pub fn hide(&self) {
        self.window.hide()
    }

    /// Returns the position of the top-left hand corner of the window relative to the
    ///  top-left hand corner of the desktop.
    ///
    /// Note that the top-left hand corner of the desktop is not necessarly the same as
    ///  the screen. If the user uses a desktop with multiple monitors, the top-left hand corner
    ///  of the desktop is the top-left hand corner of the monitor at the top-left of the desktop.
    ///
    /// The coordinates can be negative if the top-left hand corner of the window is outside
    ///  of the visible screen region.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        self.window.get_position()
    }

    /// Modifies the position of the window.
    ///
    /// See `get_position` for more informations about the coordinates.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_position(&self, x: i32, y: i32) {
        self.window.set_position(x, y)
    }

    /// Returns the size in pixels of the client area of the window.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    /// These are the dimensions of the frame buffer, and the dimensions that you should use
    ///  when you call `glViewport`.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        self.window.get_inner_size()
    }

    /// Returns the size in pixels of the window.
    ///
    /// These dimensions include title bar and borders. If you don't want these, you should use
    ///  use `get_inner_size` instead.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.window.get_outer_size()
    }

    /// Modifies the inner size of the window.
    ///
    /// See `get_inner_size` for more informations about the values.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        self.window.set_inner_size(x, y)
    }

    /// Returns an iterator to all the events that are currently in the window's events queue.
    ///
    /// Contrary to `wait_events`, this function never blocks.
    #[inline]
    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator { data: self.window.poll_events().into_iter() }
    }

    /// Waits for an event, then returns an iterator to all the events that are currently
    ///  in the window's events queue.
    ///
    /// If there are no events in queue when you call the function,
    ///  this function will block until there is one.
    #[inline]
    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator { data: self.window.wait_events().into_iter() }
    }

    /// Sets the context as the current context.
    #[inline]
    pub unsafe fn make_current(&self) {
        self.window.make_current()
    }

    /// Returns the address of an OpenGL function.
    ///
    /// Contrary to `wglGetProcAddress`, all available OpenGL functions return an address.
    #[inline]
    pub fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        self.window.get_proc_address(addr) as *const libc::c_void
    }

    /// Swaps the buffers in case of double or triple buffering.
    ///
    /// You should call this function every time you have finished rendering, or the image
    ///  may not be displayed on the screen.
    ///
    /// **Warning**: if you enabled vsync, this function will block until the next time the screen
    /// is refreshed. However drivers can choose to override your vsync settings, which means that
    /// you can't know in advance whether `swap_buffers` will block or not.
    #[inline]
    pub fn swap_buffers(&self) {
        self.window.swap_buffers()
    }

    /// Gets the native platform specific display for this window.
    /// This is typically only required when integrating with
    /// other libraries that need this information.
    #[inline]
    pub unsafe fn platform_display(&self) -> *mut libc::c_void {
        self.window.platform_display()
    }

    /// Returns the API that is currently provided by this window.
    ///
    /// - On Windows and OS/X, this always returns `OpenGl`.
    /// - On Android, this always returns `OpenGlEs`.
    /// - On Linux, it must be checked at runtime.
    pub fn get_api(&self) -> Api {
        self.window.get_api()
    }

    /// Create a window proxy for this window, that can be freely
    /// passed to different threads.
    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy {
            proxy: self.window.create_window_proxy()
        }
    }

    /// Sets a resize callback that is called by Mac (and potentially other
    /// operating systems) during resize operations. This can be used to repaint
    /// during window resizing.
    #[experimental]
    pub fn set_window_resize_callback(&mut self, callback: Option<fn(u32, u32)>) {
        self.window.set_window_resize_callback(callback);
    }

    /// Modifies the mouse cursor of the window.
    /// Has no effect on Android.
    pub fn set_cursor(&mut self, cursor: MouseCursor) {
        self.window.set_cursor(cursor);
    }
}

#[cfg(feature = "window")]
impl gl_common::GlFunctionsSource for Window {
    fn get_proc_addr(&self, addr: &str) -> *const libc::c_void {
        self.get_proc_address(addr)
    }
}

/// Represents a thread safe subset of operations that can be called
/// on a window. This structure can be safely cloned and sent between
/// threads.
///
#[cfg(feature = "window")]
#[derive(Clone)]
pub struct WindowProxy {
    proxy: winimpl::WindowProxy,
}

#[cfg(feature = "window")]
impl WindowProxy {

    /// Triggers a blocked event loop to wake up. This is
    /// typically called when another thread wants to wake
    /// up the blocked rendering thread to cause a refresh.
    #[inline]
    pub fn wakeup_event_loop(&self) {
        self.proxy.wakeup_event_loop();
    }
}

/// Represents a headless OpenGL context.
#[cfg(feature = "headless")]
pub struct HeadlessContext {
    context: winimpl::HeadlessContext,
}

#[cfg(feature = "headless")]
impl HeadlessContext {
    /// Creates a new OpenGL context
    /// Sets the context as the current context.
    #[inline]
    pub unsafe fn make_current(&self) {
        self.context.make_current()
    }

    /// Returns the address of an OpenGL function.
    ///
    /// Contrary to `wglGetProcAddress`, all available OpenGL functions return an address.
    #[inline]
    pub fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        self.context.get_proc_address(addr) as *const libc::c_void
    }

    /// Returns the API that is currently provided by this window.
    ///
    /// See `Window::get_api` for more infos.
    pub fn get_api(&self) -> Api {
        self.context.get_api()
    }

    #[experimental]
    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }
}

#[cfg(feature = "headless")]
impl gl_common::GlFunctionsSource for HeadlessContext {
    fn get_proc_addr(&self, addr: &str) -> *const libc::c_void {
        self.get_proc_address(addr)
    }
}

/// An iterator for the `poll_events` function.
// Implementation note: we retreive the list once, then serve each element by one by one.
// This may change in the future.
pub struct PollEventsIterator<'a> {
    data: RingBufIter<Event>,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;
    fn next(&mut self) -> Option<Event> {
        self.data.next()
    }
}

/// An iterator for the `wait_events` function.
// Implementation note: we retreive the list once, then serve each element by one by one.
// This may change in the future.
pub struct WaitEventsIterator<'a> {
    data: RingBufIter<Event>,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;
    fn next(&mut self) -> Option<Event> {
        self.data.next()
    }
}

/// An iterator for the list of available monitors.
// Implementation note: we retreive the list once, then serve each element by one by one.
// This may change in the future.
#[cfg(feature = "window")]
pub struct AvailableMonitorsIter {
    data: RingBufIter<winimpl::MonitorID>,
}

#[cfg(feature = "window")]
impl Iterator for AvailableMonitorsIter {
    type Item = MonitorID;
    fn next(&mut self) -> Option<MonitorID> {
        self.data.next().map(|id| MonitorID(id))
    }
}

/// Returns the list of all available monitors.
#[cfg(feature = "window")]
pub fn get_available_monitors() -> AvailableMonitorsIter {
    let data = winimpl::get_available_monitors();
    AvailableMonitorsIter{ data: data.into_iter() }
}

/// Returns the primary monitor of the system.
#[cfg(feature = "window")]
pub fn get_primary_monitor() -> MonitorID {
    MonitorID(winimpl::get_primary_monitor())
}

#[cfg(feature = "window")]
impl MonitorID {
    /// Returns a human-readable name of the monitor.
    pub fn get_name(&self) -> Option<String> {
        let &MonitorID(ref id) = self;
        id.get_name()
    }

    /// Returns the number of pixels currently displayed on the monitor.
    pub fn get_dimensions(&self) -> (u32, u32) {
        let &MonitorID(ref id) = self;
        id.get_dimensions()
    }
}
