#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]

use std::collections::VecDeque;
use std::{env, mem};
use std::ffi::CStr;
use std::os::raw::*;
use std::sync::Arc;

use parking_lot::Mutex;
use sctk::reexports::client::ConnectError;

use {
    CreationError,
    EventsLoopClosed,
    Icon,
    MouseCursor,
    ControlFlow,
    WindowAttributes,
};
use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use window::MonitorId as RootMonitorId;
use self::x11::{XConnection, XError};
use self::x11::ffi::XVisualInfo;
pub use self::x11::XNotSupported;

mod dlopen;
pub mod wayland;
pub mod x11;

/// Environment variable specifying which backend should be used on unix platform.
///
/// Legal values are x11 and wayland. If this variable is set only the named backend
/// will be tried by winit. If it is not set, winit will try to connect to a wayland connection,
/// and if it fails will fallback on x11.
///
/// If this variable is set with any other value, winit will panic.
const BACKEND_PREFERENCE_ENV_VAR: &str = "WINIT_UNIX_BACKEND";

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub visual_infos: Option<XVisualInfo>,
    pub screen_id: Option<i32>,
    pub resize_increments: Option<(u32, u32)>,
    pub base_size: Option<(u32, u32)>,
    pub class: Option<(String, String)>,
    pub override_redirect: bool,
    pub x11_window_type: x11::util::WindowType,
    pub gtk_theme_variant: Option<String>,
}

lazy_static!(
    pub static ref X11_BACKEND: Mutex<Result<Arc<XConnection>, XNotSupported>> = {
        Mutex::new(XConnection::new(Some(x_error_callback)).map(Arc::new))
    };
);

pub enum Window {
    X(x11::Window),
    Wayland(wayland::Window),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WindowId {
    X(x11::WindowId),
    Wayland(wayland::WindowId),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceId {
    X(x11::DeviceId),
    Wayland(wayland::DeviceId),
}

#[derive(Debug, Clone)]
pub enum MonitorId {
    X(x11::MonitorId),
    Wayland(wayland::MonitorId),
}

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        match self {
            &MonitorId::X(ref m) => m.get_name(),
            &MonitorId::Wayland(ref m) => m.get_name(),
        }
    }

    #[inline]
    pub fn get_native_identifier(&self) -> u32 {
        match self {
            &MonitorId::X(ref m) => m.get_native_identifier(),
            &MonitorId::Wayland(ref m) => m.get_native_identifier(),
        }
    }

    #[inline]
    pub fn get_dimensions(&self) -> PhysicalSize {
        match self {
            &MonitorId::X(ref m) => m.get_dimensions(),
            &MonitorId::Wayland(ref m) => m.get_dimensions(),
        }
    }

    #[inline]
    pub fn get_position(&self) -> PhysicalPosition {
        match self {
            &MonitorId::X(ref m) => m.get_position(),
            &MonitorId::Wayland(ref m) => m.get_position(),
        }
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        match self {
            &MonitorId::X(ref m) => m.get_hidpi_factor(),
            &MonitorId::Wayland(ref m) => m.get_hidpi_factor() as f64,
        }
    }
}

impl Window {
    #[inline]
    pub fn new(
        events_loop: &EventsLoop,
        attribs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, CreationError> {
        match *events_loop {
            EventsLoop::Wayland(ref events_loop) => {
                wayland::Window::new(events_loop, attribs).map(Window::Wayland)
            },
            EventsLoop::X(ref events_loop) => {
                x11::Window::new(events_loop, attribs, pl_attribs).map(Window::X)
            },
        }
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        match self {
            &Window::X(ref w) => WindowId::X(w.id()),
            &Window::Wayland(ref w) => WindowId::Wayland(w.id()),
        }
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        match self {
            &Window::X(ref w) => w.set_title(title),
            &Window::Wayland(ref w) => w.set_title(title),
        }
    }

    #[inline]
    pub fn show(&self) {
        match self {
            &Window::X(ref w) => w.show(),
            &Window::Wayland(ref w) => w.show(),
        }
    }

    #[inline]
    pub fn hide(&self) {
        match self {
            &Window::X(ref w) => w.hide(),
            &Window::Wayland(ref w) => w.hide(),
        }
    }

    #[inline]
    pub fn get_position(&self) -> Option<LogicalPosition> {
        match self {
            &Window::X(ref w) => w.get_position(),
            &Window::Wayland(ref w) => w.get_position(),
        }
    }

    #[inline]
    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        match self {
            &Window::X(ref m) => m.get_inner_position(),
            &Window::Wayland(ref m) => m.get_inner_position(),
        }
    }

    #[inline]
    pub fn set_position(&self, position: LogicalPosition) {
        match self {
            &Window::X(ref w) => w.set_position(position),
            &Window::Wayland(ref w) => w.set_position(position),
        }
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        match self {
            &Window::X(ref w) => w.get_inner_size(),
            &Window::Wayland(ref w) => w.get_inner_size(),
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        match self {
            &Window::X(ref w) => w.get_outer_size(),
            &Window::Wayland(ref w) => w.get_outer_size(),
        }
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        match self {
            &Window::X(ref w) => w.set_inner_size(size),
            &Window::Wayland(ref w) => w.set_inner_size(size),
        }
    }

    #[inline]
    pub fn set_min_dimensions(&self, dimensions: Option<LogicalSize>) {
        match self {
            &Window::X(ref w) => w.set_min_dimensions(dimensions),
            &Window::Wayland(ref w) => w.set_min_dimensions(dimensions),
        }
    }

    #[inline]
    pub fn set_max_dimensions(&self, dimensions: Option<LogicalSize>) {
        match self {
            &Window::X(ref w) => w.set_max_dimensions(dimensions),
            &Window::Wayland(ref w) => w.set_max_dimensions(dimensions),
        }
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        match self {
            &Window::X(ref w) => w.set_resizable(resizable),
            &Window::Wayland(ref w) => w.set_resizable(resizable),
        }
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        match self {
            &Window::X(ref w) => w.set_cursor(cursor),
            &Window::Wayland(ref w) => w.set_cursor(cursor)
        }
    }

    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        match self {
            &Window::X(ref window) => window.grab_cursor(grab),
            &Window::Wayland(ref window) => window.grab_cursor(grab),
        }
    }

    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        match self {
            &Window::X(ref window) => window.hide_cursor(hide),
            &Window::Wayland(ref window) => window.hide_cursor(hide),
        }
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
       match self {
            &Window::X(ref w) => w.get_hidpi_factor(),
            &Window::Wayland(ref w) => w.hidpi_factor() as f64,
        }
    }

    #[inline]
    pub fn set_cursor_position(&self, position: LogicalPosition) -> Result<(), String> {
        match self {
            &Window::X(ref w) => w.set_cursor_position(position),
            &Window::Wayland(ref w) => w.set_cursor_position(position),
        }
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        match self {
            &Window::X(ref w) => w.set_maximized(maximized),
            &Window::Wayland(ref w) => w.set_maximized(maximized),
        }
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<RootMonitorId>) {
        match self {
            &Window::X(ref w) => w.set_fullscreen(monitor),
            &Window::Wayland(ref w) => w.set_fullscreen(monitor)
        }
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        match self {
            &Window::X(ref w) => w.set_decorations(decorations),
            &Window::Wayland(ref w) => w.set_decorations(decorations)
        }
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        match self {
            &Window::X(ref w) => w.set_always_on_top(always_on_top),
            &Window::Wayland(_) => (),
        }
    }

    #[inline]
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        match self {
            &Window::X(ref w) => w.set_window_icon(window_icon),
            &Window::Wayland(_) => (),
        }
    }

    #[inline]
    pub fn set_ime_spot(&self, position: LogicalPosition) {
        match self {
            &Window::X(ref w) => w.set_ime_spot(position),
            &Window::Wayland(_) => (),
        }
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        match self {
            &Window::X(ref window) => RootMonitorId { inner: MonitorId::X(window.get_current_monitor()) },
            &Window::Wayland(ref window) => RootMonitorId { inner: MonitorId::Wayland(window.get_current_monitor()) },
        }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        match self {
            &Window::X(ref window) => window.get_available_monitors()
                .into_iter()
                .map(MonitorId::X)
                .collect(),
            &Window::Wayland(ref window) => window.get_available_monitors()
                .into_iter()
                .map(MonitorId::Wayland)
                .collect(),
        }
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        match self {
            &Window::X(ref window) => MonitorId::X(window.get_primary_monitor()),
            &Window::Wayland(ref window) => MonitorId::Wayland(window.get_primary_monitor()),
        }
    }
}

unsafe extern "C" fn x_error_callback(
    display: *mut x11::ffi::Display,
    event: *mut x11::ffi::XErrorEvent,
) -> c_int {
    let xconn_lock = X11_BACKEND.lock();
    if let Ok(ref xconn) = *xconn_lock {
        let mut buf: [c_char; 1024] = mem::uninitialized();
        (xconn.xlib.XGetErrorText)(
            display,
            (*event).error_code as c_int,
            buf.as_mut_ptr(),
            buf.len() as c_int,
        );
        let description = CStr::from_ptr(buf.as_ptr()).to_string_lossy();

        let error = XError {
            description: description.into_owned(),
            error_code: (*event).error_code,
            request_code: (*event).request_code,
            minor_code: (*event).minor_code,
        };

        eprintln!("[winit X11 error] {:#?}", error);

        *xconn.latest_error.lock() = Some(error);
    }
    // Fun fact: this return value is completely ignored.
    0
}

pub enum EventsLoop {
    Wayland(wayland::EventsLoop),
    X(x11::EventsLoop)
}

#[derive(Clone)]
pub enum EventsLoopProxy {
    X(x11::EventsLoopProxy),
    Wayland(wayland::EventsLoopProxy),
}

impl EventsLoop {
    pub fn new() -> EventsLoop {
        if let Ok(env_var) = env::var(BACKEND_PREFERENCE_ENV_VAR) {
            match env_var.as_str() {
                "x11" => {
                    // TODO: propagate
                    return EventsLoop::new_x11().expect("Failed to initialize X11 backend");
                },
                "wayland" => {
                    return EventsLoop::new_wayland()
                        .expect("Failed to initialize Wayland backend");
                },
                _ => panic!(
                    "Unknown environment variable value for {}, try one of `x11`,`wayland`",
                    BACKEND_PREFERENCE_ENV_VAR,
                ),
            }
        }

        let wayland_err = match EventsLoop::new_wayland() {
            Ok(event_loop) => return event_loop,
            Err(err) => err,
        };

        let x11_err = match EventsLoop::new_x11() {
            Ok(event_loop) => return event_loop,
            Err(err) => err,
        };

        let err_string = format!(
r#"Failed to initialize any backend!
    Wayland status: {:#?}
    X11 status: {:#?}
"#,
            wayland_err,
            x11_err,
        );
        panic!(err_string);
    }

    pub fn new_wayland() -> Result<EventsLoop, ConnectError> {
        wayland::EventsLoop::new()
            .map(EventsLoop::Wayland)
    }

    pub fn new_x11() -> Result<EventsLoop, XNotSupported> {
        X11_BACKEND
            .lock()
            .as_ref()
            .map(Arc::clone)
            .map(x11::EventsLoop::new)
            .map(EventsLoop::X)
            .map_err(|err| err.clone())
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        match *self {
            EventsLoop::Wayland(ref evlp) => evlp
                .get_available_monitors()
                .into_iter()
                .map(MonitorId::Wayland)
                .collect(),
            EventsLoop::X(ref evlp) => evlp
                .x_connection()
                .get_available_monitors()
                .into_iter()
                .map(MonitorId::X)
                .collect(),
        }
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        match *self {
            EventsLoop::Wayland(ref evlp) => MonitorId::Wayland(evlp.get_primary_monitor()),
            EventsLoop::X(ref evlp) => MonitorId::X(evlp.x_connection().get_primary_monitor()),
        }
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        match *self {
            EventsLoop::Wayland(ref evlp) => EventsLoopProxy::Wayland(evlp.create_proxy()),
            EventsLoop::X(ref evlp) => EventsLoopProxy::X(evlp.create_proxy()),
        }
    }

    pub fn poll_events<F>(&mut self, callback: F)
        where F: FnMut(::Event)
    {
        match *self {
            EventsLoop::Wayland(ref mut evlp) => evlp.poll_events(callback),
            EventsLoop::X(ref mut evlp) => evlp.poll_events(callback)
        }
    }

    pub fn run_forever<F>(&mut self, callback: F)
        where F: FnMut(::Event) -> ControlFlow
    {
        match *self {
            EventsLoop::Wayland(ref mut evlp) => evlp.run_forever(callback),
            EventsLoop::X(ref mut evlp) => evlp.run_forever(callback)
        }
    }

    #[inline]
    pub fn is_wayland(&self) -> bool {
        match *self {
            EventsLoop::Wayland(_) => true,
            EventsLoop::X(_) => false,
        }
    }

    #[inline]
    pub fn x_connection(&self) -> Option<&Arc<XConnection>> {
        match *self {
            EventsLoop::Wayland(_) => None,
            EventsLoop::X(ref ev) => Some(ev.x_connection()),
        }
    }
}

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        match *self {
            EventsLoopProxy::Wayland(ref proxy) => proxy.wakeup(),
            EventsLoopProxy::X(ref proxy) => proxy.wakeup(),
        }
    }
}
