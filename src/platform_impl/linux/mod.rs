#![cfg(free_unix)]

#[cfg(all(not(x11_platform), not(wayland_platform)))]
compile_error!("Please select a feature to build for unix: `x11`, `wayland`");

#[cfg(wayland_platform)]
use std::error::Error;

use std::{collections::VecDeque, env, fmt};
#[cfg(x11_platform)]
use std::{
    ffi::CStr,
    mem::MaybeUninit,
    os::raw::*,
    sync::{Arc, Mutex},
};

#[cfg(x11_platform)]
use once_cell::sync::Lazy;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

#[cfg(x11_platform)]
pub use self::x11::XNotSupported;
#[cfg(x11_platform)]
use self::x11::{ffi::XVisualInfo, util::WindowType as XWindowType, XConnection, XError};
#[cfg(x11_platform)]
use crate::platform::x11::XlibErrorHook;
use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    event::Event,
    event_loop::{
        ControlFlow, DeviceEventFilter, EventLoopClosed, EventLoopWindowTarget as RootELW,
    },
    icon::Icon,
    window::{
        CursorGrabMode, CursorIcon, ImePurpose, ResizeDirection, Theme, UserAttentionType,
        WindowAttributes, WindowButtons, WindowLevel,
    },
};

pub(crate) use crate::icon::RgbaIcon as PlatformIcon;
pub(self) use crate::platform_impl::Fullscreen;

#[cfg(wayland_platform)]
pub mod wayland;
#[cfg(x11_platform)]
pub mod x11;

/// Environment variable specifying which backend should be used on unix platform.
///
/// Legal values are x11 and wayland. If this variable is set only the named backend
/// will be tried by winit. If it is not set, winit will try to connect to a wayland connection,
/// and if it fails will fallback on x11.
///
/// If this variable is set with any other value, winit will panic.
const BACKEND_PREFERENCE_ENV_VAR: &str = "WINIT_UNIX_BACKEND";

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Backend {
    #[cfg(x11_platform)]
    X,
    #[cfg(wayland_platform)]
    Wayland,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {
    pub(crate) forced_backend: Option<Backend>,
    pub(crate) any_thread: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationName {
    pub general: String,
    pub instance: String,
}

impl ApplicationName {
    pub fn new(general: String, instance: String) -> Self {
        Self { general, instance }
    }
}

#[derive(Clone)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub name: Option<ApplicationName>,
    #[cfg(x11_platform)]
    pub visual_infos: Option<XVisualInfo>,
    #[cfg(x11_platform)]
    pub screen_id: Option<i32>,
    #[cfg(x11_platform)]
    pub base_size: Option<Size>,
    #[cfg(x11_platform)]
    pub override_redirect: bool,
    #[cfg(x11_platform)]
    pub x11_window_types: Vec<XWindowType>,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> Self {
        Self {
            name: None,
            #[cfg(x11_platform)]
            visual_infos: None,
            #[cfg(x11_platform)]
            screen_id: None,
            #[cfg(x11_platform)]
            base_size: None,
            #[cfg(x11_platform)]
            override_redirect: false,
            #[cfg(x11_platform)]
            x11_window_types: vec![XWindowType::Normal],
        }
    }
}

#[cfg(x11_platform)]
pub(crate) static X11_BACKEND: Lazy<Mutex<Result<Arc<XConnection>, XNotSupported>>> =
    Lazy::new(|| Mutex::new(XConnection::new(Some(x_error_callback)).map(Arc::new)));

#[derive(Debug, Clone)]
pub enum OsError {
    #[cfg(x11_platform)]
    XError(XError),
    #[cfg(x11_platform)]
    XMisc(&'static str),
    #[cfg(wayland_platform)]
    WaylandMisc(&'static str),
}

impl fmt::Display for OsError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            #[cfg(x11_platform)]
            OsError::XError(ref e) => _f.pad(&e.description),
            #[cfg(x11_platform)]
            OsError::XMisc(e) => _f.pad(e),
            #[cfg(wayland_platform)]
            OsError::WaylandMisc(e) => _f.pad(e),
        }
    }
}

pub(crate) enum Window {
    #[cfg(x11_platform)]
    X(x11::Window),
    #[cfg(wayland_platform)]
    Wayland(wayland::Window),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(u64);

impl From<WindowId> for u64 {
    fn from(window_id: WindowId) -> Self {
        window_id.0
    }
}

impl From<u64> for WindowId {
    fn from(raw_id: u64) -> Self {
        Self(raw_id)
    }
}

impl WindowId {
    pub const unsafe fn dummy() -> Self {
        Self(0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceId {
    #[cfg(x11_platform)]
    X(x11::DeviceId),
    #[cfg(wayland_platform)]
    Wayland(wayland::DeviceId),
}

impl DeviceId {
    pub const unsafe fn dummy() -> Self {
        #[cfg(wayland_platform)]
        return DeviceId::Wayland(wayland::DeviceId::dummy());
        #[cfg(all(not(wayland_platform), x11_platform))]
        return DeviceId::X(x11::DeviceId::dummy());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MonitorHandle {
    #[cfg(x11_platform)]
    X(x11::MonitorHandle),
    #[cfg(wayland_platform)]
    Wayland(wayland::MonitorHandle),
}

/// `x11_or_wayland!(match expr; Enum(foo) => foo.something())`
/// expands to the equivalent of
/// ```ignore
/// match self {
///    Enum::X(foo) => foo.something(),
///    Enum::Wayland(foo) => foo.something(),
/// }
/// ```
/// The result can be converted to another enum by adding `; as AnotherEnum`
macro_rules! x11_or_wayland {
    (match $what:expr; $enum:ident ( $($c1:tt)* ) => $x:expr; as $enum2:ident ) => {
        match $what {
            #[cfg(x11_platform)]
            $enum::X($($c1)*) => $enum2::X($x),
            #[cfg(wayland_platform)]
            $enum::Wayland($($c1)*) => $enum2::Wayland($x),
        }
    };
    (match $what:expr; $enum:ident ( $($c1:tt)* ) => $x:expr) => {
        match $what {
            #[cfg(x11_platform)]
            $enum::X($($c1)*) => $x,
            #[cfg(wayland_platform)]
            $enum::Wayland($($c1)*) => $x,
        }
    };
}

impl MonitorHandle {
    #[inline]
    pub fn name(&self) -> Option<String> {
        x11_or_wayland!(match self; MonitorHandle(m) => m.name())
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        x11_or_wayland!(match self; MonitorHandle(m) => m.native_identifier())
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        x11_or_wayland!(match self; MonitorHandle(m) => m.size())
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        x11_or_wayland!(match self; MonitorHandle(m) => m.position())
    }

    #[inline]
    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        x11_or_wayland!(match self; MonitorHandle(m) => m.refresh_rate_millihertz())
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        x11_or_wayland!(match self; MonitorHandle(m) => m.scale_factor() as _)
    }

    #[inline]
    pub fn video_modes(&self) -> Box<dyn Iterator<Item = VideoMode>> {
        x11_or_wayland!(match self; MonitorHandle(m) => Box::new(m.video_modes()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VideoMode {
    #[cfg(x11_platform)]
    X(x11::VideoMode),
    #[cfg(wayland_platform)]
    Wayland(wayland::VideoMode),
}

impl VideoMode {
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        x11_or_wayland!(match self; VideoMode(m) => m.size())
    }

    #[inline]
    pub fn bit_depth(&self) -> u16 {
        x11_or_wayland!(match self; VideoMode(m) => m.bit_depth())
    }

    #[inline]
    pub fn refresh_rate_millihertz(&self) -> u32 {
        x11_or_wayland!(match self; VideoMode(m) => m.refresh_rate_millihertz())
    }

    #[inline]
    pub fn monitor(&self) -> MonitorHandle {
        x11_or_wayland!(match self; VideoMode(m) => m.monitor())
    }
}

impl Window {
    #[inline]
    pub(crate) fn new<T>(
        window_target: &EventLoopWindowTarget<T>,
        attribs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, RootOsError> {
        match *window_target {
            #[cfg(wayland_platform)]
            EventLoopWindowTarget::Wayland(ref window_target) => {
                wayland::Window::new(window_target, attribs, pl_attribs).map(Window::Wayland)
            }
            #[cfg(x11_platform)]
            EventLoopWindowTarget::X(ref window_target) => {
                x11::Window::new(window_target, attribs, pl_attribs).map(Window::X)
            }
        }
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        match self {
            #[cfg(wayland_platform)]
            Self::Wayland(window) => window.id(),
            #[cfg(x11_platform)]
            Self::X(window) => window.id(),
        }
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        x11_or_wayland!(match self; Window(w) => w.set_title(title));
    }

    #[inline]
    pub fn set_transparent(&self, transparent: bool) {
        x11_or_wayland!(match self; Window(w) => w.set_transparent(transparent));
    }

    #[inline]
    pub fn set_visible(&self, visible: bool) {
        x11_or_wayland!(match self; Window(w) => w.set_visible(visible))
    }

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        x11_or_wayland!(match self; Window(w) => w.is_visible())
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        x11_or_wayland!(match self; Window(w) => w.outer_position())
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        x11_or_wayland!(match self; Window(w) => w.inner_position())
    }

    #[inline]
    pub fn set_outer_position(&self, position: Position) {
        x11_or_wayland!(match self; Window(w) => w.set_outer_position(position))
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        x11_or_wayland!(match self; Window(w) => w.inner_size())
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        x11_or_wayland!(match self; Window(w) => w.outer_size())
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        x11_or_wayland!(match self; Window(w) => w.set_inner_size(size))
    }

    #[inline]
    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        x11_or_wayland!(match self; Window(w) => w.set_min_inner_size(dimensions))
    }

    #[inline]
    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        x11_or_wayland!(match self; Window(w) => w.set_max_inner_size(dimensions))
    }

    #[inline]
    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        x11_or_wayland!(match self; Window(w) => w.resize_increments())
    }

    #[inline]
    pub fn set_resize_increments(&self, increments: Option<Size>) {
        x11_or_wayland!(match self; Window(w) => w.set_resize_increments(increments))
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        x11_or_wayland!(match self; Window(w) => w.set_resizable(resizable))
    }

    #[inline]
    pub fn is_resizable(&self) -> bool {
        x11_or_wayland!(match self; Window(w) => w.is_resizable())
    }

    #[inline]
    pub fn set_enabled_buttons(&self, buttons: WindowButtons) {
        x11_or_wayland!(match self; Window(w) => w.set_enabled_buttons(buttons))
    }

    #[inline]
    pub fn enabled_buttons(&self) -> WindowButtons {
        x11_or_wayland!(match self; Window(w) => w.enabled_buttons())
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        x11_or_wayland!(match self; Window(w) => w.set_cursor_icon(cursor))
    }

    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        x11_or_wayland!(match self; Window(window) => window.set_cursor_grab(mode))
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        x11_or_wayland!(match self; Window(window) => window.set_cursor_visible(visible))
    }

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        x11_or_wayland!(match self; Window(window) => window.drag_window())
    }

    #[inline]
    pub fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), ExternalError> {
        x11_or_wayland!(match self; Window(window) => window.drag_resize_window(direction))
    }

    #[inline]
    pub fn set_cursor_hittest(&self, hittest: bool) -> Result<(), ExternalError> {
        x11_or_wayland!(match self; Window(w) => w.set_cursor_hittest(hittest))
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        x11_or_wayland!(match self; Window(w) => w.scale_factor())
    }

    #[inline]
    pub fn set_cursor_position(&self, position: Position) -> Result<(), ExternalError> {
        x11_or_wayland!(match self; Window(w) => w.set_cursor_position(position))
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        x11_or_wayland!(match self; Window(w) => w.set_maximized(maximized))
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        x11_or_wayland!(match self; Window(w) => w.is_maximized())
    }

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        x11_or_wayland!(match self; Window(w) => w.set_minimized(minimized))
    }

    #[inline]
    pub fn is_minimized(&self) -> Option<bool> {
        x11_or_wayland!(match self; Window(w) => w.is_minimized())
    }

    #[inline]
    pub(crate) fn fullscreen(&self) -> Option<Fullscreen> {
        x11_or_wayland!(match self; Window(w) => w.fullscreen())
    }

    #[inline]
    pub(crate) fn set_fullscreen(&self, monitor: Option<Fullscreen>) {
        x11_or_wayland!(match self; Window(w) => w.set_fullscreen(monitor))
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        x11_or_wayland!(match self; Window(w) => w.set_decorations(decorations))
    }

    #[inline]
    pub fn is_decorated(&self) -> bool {
        x11_or_wayland!(match self; Window(w) => w.is_decorated())
    }

    #[inline]
    pub fn set_window_level(&self, _level: WindowLevel) {
        match self {
            #[cfg(x11_platform)]
            Window::X(ref w) => w.set_window_level(_level),
            #[cfg(wayland_platform)]
            Window::Wayland(_) => (),
        }
    }

    #[inline]
    pub fn set_window_icon(&self, _window_icon: Option<Icon>) {
        match self {
            #[cfg(x11_platform)]
            Window::X(ref w) => w.set_window_icon(_window_icon),
            #[cfg(wayland_platform)]
            Window::Wayland(_) => (),
        }
    }

    #[inline]
    pub fn set_ime_position(&self, position: Position) {
        x11_or_wayland!(match self; Window(w) => w.set_ime_position(position))
    }

    #[inline]
    pub fn set_ime_allowed(&self, allowed: bool) {
        x11_or_wayland!(match self; Window(w) => w.set_ime_allowed(allowed))
    }

    #[inline]
    pub fn set_ime_purpose(&self, purpose: ImePurpose) {
        x11_or_wayland!(match self; Window(w) => w.set_ime_purpose(purpose))
    }

    #[inline]
    pub fn focus_window(&self) {
        match self {
            #[cfg(x11_platform)]
            Window::X(ref w) => w.focus_window(),
            #[cfg(wayland_platform)]
            Window::Wayland(_) => (),
        }
    }
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        match self {
            #[cfg(x11_platform)]
            Window::X(ref w) => w.request_user_attention(request_type),
            #[cfg(wayland_platform)]
            Window::Wayland(ref w) => w.request_user_attention(request_type),
        }
    }

    #[inline]
    pub fn request_redraw(&self) {
        x11_or_wayland!(match self; Window(w) => w.request_redraw())
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        match self {
            #[cfg(x11_platform)]
            Window::X(ref window) => {
                let current_monitor = MonitorHandle::X(window.current_monitor());
                Some(current_monitor)
            }
            #[cfg(wayland_platform)]
            Window::Wayland(ref window) => {
                let current_monitor = MonitorHandle::Wayland(window.current_monitor()?);
                Some(current_monitor)
            }
        }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        match self {
            #[cfg(x11_platform)]
            Window::X(ref window) => window
                .available_monitors()
                .into_iter()
                .map(MonitorHandle::X)
                .collect(),
            #[cfg(wayland_platform)]
            Window::Wayland(ref window) => window
                .available_monitors()
                .into_iter()
                .map(MonitorHandle::Wayland)
                .collect(),
        }
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        match self {
            #[cfg(x11_platform)]
            Window::X(ref window) => {
                let primary_monitor = MonitorHandle::X(window.primary_monitor());
                Some(primary_monitor)
            }
            #[cfg(wayland_platform)]
            Window::Wayland(ref window) => window.primary_monitor(),
        }
    }

    #[inline]
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        x11_or_wayland!(match self; Window(window) => window.raw_window_handle())
    }

    #[inline]
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        x11_or_wayland!(match self; Window(window) => window.raw_display_handle())
    }

    #[inline]
    pub fn set_theme(&self, theme: Option<Theme>) {
        x11_or_wayland!(match self; Window(window) => window.set_theme(theme))
    }

    #[inline]
    pub fn theme(&self) -> Option<Theme> {
        x11_or_wayland!(match self; Window(window) => window.theme())
    }

    #[inline]
    pub fn has_focus(&self) -> bool {
        x11_or_wayland!(match self; Window(window) => window.has_focus())
    }

    pub fn title(&self) -> String {
        x11_or_wayland!(match self; Window(window) => window.title())
    }
}

/// Hooks for X11 errors.
#[cfg(x11_platform)]
pub(crate) static mut XLIB_ERROR_HOOKS: Lazy<Mutex<Vec<XlibErrorHook>>> =
    Lazy::new(|| Mutex::new(Vec::new()));

#[cfg(x11_platform)]
unsafe extern "C" fn x_error_callback(
    display: *mut x11::ffi::Display,
    event: *mut x11::ffi::XErrorEvent,
) -> c_int {
    let xconn_lock = X11_BACKEND.lock().unwrap();
    if let Ok(ref xconn) = *xconn_lock {
        // Call all the hooks.
        let mut error_handled = false;
        for hook in XLIB_ERROR_HOOKS.lock().unwrap().iter() {
            error_handled |= hook(display as *mut _, event as *mut _);
        }

        // `assume_init` is safe here because the array consists of `MaybeUninit` values,
        // which do not require initialization.
        let mut buf: [MaybeUninit<c_char>; 1024] = MaybeUninit::uninit().assume_init();
        (xconn.xlib.XGetErrorText)(
            display,
            (*event).error_code as c_int,
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as c_int,
        );
        let description = CStr::from_ptr(buf.as_ptr() as *const c_char).to_string_lossy();

        let error = XError {
            description: description.into_owned(),
            error_code: (*event).error_code,
            request_code: (*event).request_code,
            minor_code: (*event).minor_code,
        };

        // Don't log error.
        if !error_handled {
            error!("X11 error: {:#?}", error);
            // XXX only update the error, if it wasn't handled by any of the hooks.
            *xconn.latest_error.lock().unwrap() = Some(error);
        }
    }
    // Fun fact: this return value is completely ignored.
    0
}

pub enum EventLoop<T: 'static> {
    #[cfg(wayland_platform)]
    Wayland(Box<wayland::EventLoop<T>>),
    #[cfg(x11_platform)]
    X(x11::EventLoop<T>),
}

pub enum EventLoopProxy<T: 'static> {
    #[cfg(x11_platform)]
    X(x11::EventLoopProxy<T>),
    #[cfg(wayland_platform)]
    Wayland(wayland::EventLoopProxy<T>),
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        x11_or_wayland!(match self; EventLoopProxy(proxy) => proxy.clone(); as EventLoopProxy)
    }
}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(attributes: &PlatformSpecificEventLoopAttributes) -> Self {
        if !attributes.any_thread && !is_main_thread() {
            panic!(
                "Initializing the event loop outside of the main thread is a significant \
                 cross-platform compatibility hazard. If you absolutely need to create an \
                 EventLoop on a different thread, you can use the \
                 `EventLoopBuilderExtUnix::any_thread` function."
            );
        }

        #[cfg(x11_platform)]
        if attributes.forced_backend == Some(Backend::X) {
            // TODO: Propagate
            return EventLoop::new_x11_any_thread().unwrap();
        }

        #[cfg(wayland_platform)]
        if attributes.forced_backend == Some(Backend::Wayland) {
            // TODO: Propagate
            return EventLoop::new_wayland_any_thread().expect("failed to open Wayland connection");
        }

        if let Ok(env_var) = env::var(BACKEND_PREFERENCE_ENV_VAR) {
            match env_var.as_str() {
                "x11" => {
                    // TODO: propagate
                    #[cfg(x11_platform)]
                    return EventLoop::new_x11_any_thread()
                        .expect("Failed to initialize X11 backend");
                    #[cfg(not(x11_platform))]
                    panic!("x11 feature is not enabled")
                }
                "wayland" => {
                    #[cfg(wayland_platform)]
                    return EventLoop::new_wayland_any_thread()
                        .expect("Failed to initialize Wayland backend");
                    #[cfg(not(wayland_platform))]
                    panic!("wayland feature is not enabled");
                }
                _ => panic!(
                    "Unknown environment variable value for {BACKEND_PREFERENCE_ENV_VAR}, try one of `x11`,`wayland`",
                ),
            }
        }

        #[cfg(wayland_platform)]
        let wayland_err = match EventLoop::new_wayland_any_thread() {
            Ok(event_loop) => return event_loop,
            Err(err) => err,
        };

        #[cfg(x11_platform)]
        let x11_err = match EventLoop::new_x11_any_thread() {
            Ok(event_loop) => return event_loop,
            Err(err) => err,
        };

        #[cfg(not(wayland_platform))]
        let wayland_err = "backend disabled";
        #[cfg(not(x11_platform))]
        let x11_err = "backend disabled";

        panic!(
            "Failed to initialize any backend! Wayland status: {wayland_err:?} X11 status: {x11_err:?}",
        );
    }

    #[cfg(wayland_platform)]
    fn new_wayland_any_thread() -> Result<EventLoop<T>, Box<dyn Error>> {
        wayland::EventLoop::new().map(|evlp| EventLoop::Wayland(Box::new(evlp)))
    }

    #[cfg(x11_platform)]
    fn new_x11_any_thread() -> Result<EventLoop<T>, XNotSupported> {
        let xconn = match X11_BACKEND.lock().unwrap().as_ref() {
            Ok(xconn) => xconn.clone(),
            Err(err) => return Err(err.clone()),
        };

        Ok(EventLoop::X(x11::EventLoop::new(xconn)))
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        x11_or_wayland!(match self; EventLoop(evlp) => evlp.create_proxy(); as EventLoopProxy)
    }

    pub fn run_return<F>(&mut self, callback: F) -> i32
    where
        F: FnMut(crate::event::Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        x11_or_wayland!(match self; EventLoop(evlp) => evlp.run_return(callback))
    }

    pub fn run<F>(self, callback: F) -> !
    where
        F: 'static + FnMut(crate::event::Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        x11_or_wayland!(match self; EventLoop(evlp) => evlp.run(callback))
    }

    pub fn window_target(&self) -> &crate::event_loop::EventLoopWindowTarget<T> {
        x11_or_wayland!(match self; EventLoop(evlp) => evlp.window_target())
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        x11_or_wayland!(match self; EventLoopProxy(proxy) => proxy.send_event(event))
    }
}

pub enum EventLoopWindowTarget<T> {
    #[cfg(wayland_platform)]
    Wayland(wayland::EventLoopWindowTarget<T>),
    #[cfg(x11_platform)]
    X(x11::EventLoopWindowTarget<T>),
}

impl<T> EventLoopWindowTarget<T> {
    #[inline]
    pub fn is_wayland(&self) -> bool {
        match *self {
            #[cfg(wayland_platform)]
            EventLoopWindowTarget::Wayland(_) => true,
            #[cfg(x11_platform)]
            _ => false,
        }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        match *self {
            #[cfg(wayland_platform)]
            EventLoopWindowTarget::Wayland(ref evlp) => evlp
                .available_monitors()
                .into_iter()
                .map(MonitorHandle::Wayland)
                .collect(),
            #[cfg(x11_platform)]
            EventLoopWindowTarget::X(ref evlp) => evlp
                .x_connection()
                .available_monitors()
                .into_iter()
                .map(MonitorHandle::X)
                .collect(),
        }
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        match *self {
            #[cfg(wayland_platform)]
            EventLoopWindowTarget::Wayland(ref evlp) => evlp.primary_monitor(),
            #[cfg(x11_platform)]
            EventLoopWindowTarget::X(ref evlp) => {
                let primary_monitor = MonitorHandle::X(evlp.x_connection().primary_monitor());
                Some(primary_monitor)
            }
        }
    }

    #[inline]
    pub fn set_device_event_filter(&self, _filter: DeviceEventFilter) {
        match *self {
            #[cfg(wayland_platform)]
            EventLoopWindowTarget::Wayland(_) => (),
            #[cfg(x11_platform)]
            EventLoopWindowTarget::X(ref evlp) => evlp.set_device_event_filter(_filter),
        }
    }

    pub fn raw_display_handle(&self) -> raw_window_handle::RawDisplayHandle {
        x11_or_wayland!(match self; Self(evlp) => evlp.raw_display_handle())
    }
}

fn sticky_exit_callback<T, F>(
    evt: Event<'_, T>,
    target: &RootELW<T>,
    control_flow: &mut ControlFlow,
    callback: &mut F,
) where
    F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
{
    // make ControlFlow::ExitWithCode sticky by providing a dummy
    // control flow reference if it is already ExitWithCode.
    if let ControlFlow::ExitWithCode(code) = *control_flow {
        callback(evt, target, &mut ControlFlow::ExitWithCode(code))
    } else {
        callback(evt, target, control_flow)
    }
}

#[cfg(target_os = "linux")]
fn is_main_thread() -> bool {
    use libc::{c_long, getpid, syscall, SYS_gettid};

    unsafe { syscall(SYS_gettid) == getpid() as c_long }
}

#[cfg(any(target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]
fn is_main_thread() -> bool {
    use libc::pthread_main_np;

    unsafe { pthread_main_np() == 1 }
}

#[cfg(target_os = "netbsd")]
fn is_main_thread() -> bool {
    std::thread::current().name() == Some("main")
}
