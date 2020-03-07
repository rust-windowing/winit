#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]

use std::{collections::VecDeque, env, ffi::CStr, fmt, mem::MaybeUninit, os::raw::*, sync::Arc};

use parking_lot::Mutex;
use raw_window_handle::RawWindowHandle;
use smithay_client_toolkit::reexports::client::ConnectError;

pub use self::x11::XNotSupported;
use self::x11::{ffi::XVisualInfo, util::WindowType as XWindowType, XConnection, XError};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    event::Event,
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    icon::Icon,
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    window::{CursorIcon, Fullscreen, WindowAttributes},
};

pub(crate) use crate::icon::RgbaIcon as PlatformIcon;

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

#[derive(Clone)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub visual_infos: Option<XVisualInfo>,
    pub screen_id: Option<i32>,
    pub resize_increments: Option<Size>,
    pub base_size: Option<Size>,
    pub class: Option<(String, String)>,
    pub override_redirect: bool,
    pub x11_window_types: Vec<XWindowType>,
    pub gtk_theme_variant: Option<String>,
    pub app_id: Option<String>,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> Self {
        Self {
            visual_infos: None,
            screen_id: None,
            resize_increments: None,
            base_size: None,
            class: None,
            override_redirect: false,
            x11_window_types: vec![XWindowType::Normal],
            gtk_theme_variant: None,
            app_id: None,
        }
    }
}

lazy_static! {
    pub static ref X11_BACKEND: Mutex<Result<Arc<XConnection>, XNotSupported>> =
        { Mutex::new(XConnection::new(Some(x_error_callback)).map(Arc::new)) };
}

#[derive(Debug, Clone)]
pub enum OsError {
    XError(XError),
    XMisc(&'static str),
}

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            OsError::XError(e) => f.pad(&e.description),
            OsError::XMisc(e) => f.pad(e),
        }
    }
}

pub enum Window {
    X(x11::Window),
    Wayland(wayland::Window),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WindowId {
    X(x11::WindowId),
    Wayland(wayland::WindowId),
}

impl WindowId {
    pub unsafe fn dummy() -> Self {
        WindowId::Wayland(wayland::WindowId::dummy())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceId {
    X(x11::DeviceId),
    Wayland(wayland::DeviceId),
}

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        DeviceId::Wayland(wayland::DeviceId::dummy())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MonitorHandle {
    X(x11::MonitorHandle),
    Wayland(wayland::MonitorHandle),
}

impl MonitorHandle {
    #[inline]
    pub fn name(&self) -> Option<String> {
        match self {
            &MonitorHandle::X(ref m) => m.name(),
            &MonitorHandle::Wayland(ref m) => m.name(),
        }
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        match self {
            &MonitorHandle::X(ref m) => m.native_identifier(),
            &MonitorHandle::Wayland(ref m) => m.native_identifier(),
        }
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        match self {
            &MonitorHandle::X(ref m) => m.size(),
            &MonitorHandle::Wayland(ref m) => m.size(),
        }
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        match self {
            &MonitorHandle::X(ref m) => m.position(),
            &MonitorHandle::Wayland(ref m) => m.position(),
        }
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        match self {
            &MonitorHandle::X(ref m) => m.scale_factor(),
            &MonitorHandle::Wayland(ref m) => m.scale_factor() as f64,
        }
    }

    #[inline]
    pub fn video_modes(&self) -> Box<dyn Iterator<Item = RootVideoMode>> {
        match self {
            MonitorHandle::X(m) => Box::new(m.video_modes()),
            MonitorHandle::Wayland(m) => Box::new(m.video_modes()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VideoMode {
    X(x11::VideoMode),
    Wayland(wayland::VideoMode),
}

impl VideoMode {
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        match self {
            &VideoMode::X(ref m) => m.size(),
            &VideoMode::Wayland(ref m) => m.size(),
        }
    }

    #[inline]
    pub fn bit_depth(&self) -> u16 {
        match self {
            &VideoMode::X(ref m) => m.bit_depth(),
            &VideoMode::Wayland(ref m) => m.bit_depth(),
        }
    }

    #[inline]
    pub fn refresh_rate(&self) -> u16 {
        match self {
            &VideoMode::X(ref m) => m.refresh_rate(),
            &VideoMode::Wayland(ref m) => m.refresh_rate(),
        }
    }

    #[inline]
    pub fn monitor(&self) -> RootMonitorHandle {
        match self {
            &VideoMode::X(ref m) => m.monitor(),
            &VideoMode::Wayland(ref m) => m.monitor(),
        }
    }
}

impl Window {
    #[inline]
    pub fn new<T>(
        window_target: &EventLoopWindowTarget<T>,
        attribs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, RootOsError> {
        match *window_target {
            EventLoopWindowTarget::Wayland(ref window_target) => {
                wayland::Window::new(window_target, attribs, pl_attribs).map(Window::Wayland)
            }
            EventLoopWindowTarget::X(ref window_target) => {
                x11::Window::new(window_target, attribs, pl_attribs).map(Window::X)
            }
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
    pub fn set_visible(&self, visible: bool) {
        match self {
            &Window::X(ref w) => w.set_visible(visible),
            &Window::Wayland(ref w) => w.set_visible(visible),
        }
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        match self {
            &Window::X(ref w) => w.outer_position(),
            &Window::Wayland(ref w) => w.outer_position(),
        }
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        match self {
            &Window::X(ref m) => m.inner_position(),
            &Window::Wayland(ref m) => m.inner_position(),
        }
    }

    #[inline]
    pub fn set_outer_position(&self, position: Position) {
        match self {
            &Window::X(ref w) => w.set_outer_position(position),
            &Window::Wayland(ref w) => w.set_outer_position(position),
        }
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        match self {
            &Window::X(ref w) => w.inner_size(),
            &Window::Wayland(ref w) => w.inner_size(),
        }
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        match self {
            &Window::X(ref w) => w.outer_size(),
            &Window::Wayland(ref w) => w.outer_size(),
        }
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        match self {
            &Window::X(ref w) => w.set_inner_size(size),
            &Window::Wayland(ref w) => w.set_inner_size(size),
        }
    }

    #[inline]
    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        match self {
            &Window::X(ref w) => w.set_min_inner_size(dimensions),
            &Window::Wayland(ref w) => w.set_min_inner_size(dimensions),
        }
    }

    #[inline]
    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        match self {
            &Window::X(ref w) => w.set_max_inner_size(dimensions),
            &Window::Wayland(ref w) => w.set_max_inner_size(dimensions),
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
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        match self {
            &Window::X(ref w) => w.set_cursor_icon(cursor),
            &Window::Wayland(ref w) => w.set_cursor_icon(cursor),
        }
    }

    #[inline]
    pub fn set_cursor_grab(&self, grab: bool) -> Result<(), ExternalError> {
        match self {
            &Window::X(ref window) => window.set_cursor_grab(grab),
            &Window::Wayland(ref window) => window.set_cursor_grab(grab),
        }
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        match self {
            &Window::X(ref window) => window.set_cursor_visible(visible),
            &Window::Wayland(ref window) => window.set_cursor_visible(visible),
        }
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        match self {
            &Window::X(ref w) => w.scale_factor(),
            &Window::Wayland(ref w) => w.scale_factor() as f64,
        }
    }

    #[inline]
    pub fn set_cursor_position(&self, position: Position) -> Result<(), ExternalError> {
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
    pub fn set_minimized(&self, minimized: bool) {
        match self {
            &Window::X(ref w) => w.set_minimized(minimized),
            &Window::Wayland(ref w) => w.set_minimized(minimized),
        }
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        match self {
            &Window::X(ref w) => w.fullscreen(),
            &Window::Wayland(ref w) => w.fullscreen(),
        }
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<Fullscreen>) {
        match self {
            &Window::X(ref w) => w.set_fullscreen(monitor),
            &Window::Wayland(ref w) => w.set_fullscreen(monitor),
        }
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        match self {
            &Window::X(ref w) => w.set_decorations(decorations),
            &Window::Wayland(ref w) => w.set_decorations(decorations),
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
    pub fn set_ime_position(&self, position: Position) {
        match self {
            &Window::X(ref w) => w.set_ime_position(position),
            &Window::Wayland(_) => (),
        }
    }

    #[inline]
    pub fn request_redraw(&self) {
        match self {
            &Window::X(ref w) => w.request_redraw(),
            &Window::Wayland(ref w) => w.request_redraw(),
        }
    }

    #[inline]
    pub fn current_monitor(&self) -> RootMonitorHandle {
        match self {
            &Window::X(ref window) => RootMonitorHandle {
                inner: MonitorHandle::X(window.current_monitor()),
            },
            &Window::Wayland(ref window) => RootMonitorHandle {
                inner: MonitorHandle::Wayland(window.current_monitor()),
            },
        }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        match self {
            &Window::X(ref window) => window
                .available_monitors()
                .into_iter()
                .map(MonitorHandle::X)
                .collect(),
            &Window::Wayland(ref window) => window
                .available_monitors()
                .into_iter()
                .map(MonitorHandle::Wayland)
                .collect(),
        }
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        match self {
            &Window::X(ref window) => MonitorHandle::X(window.primary_monitor()),
            &Window::Wayland(ref window) => MonitorHandle::Wayland(window.primary_monitor()),
        }
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        match self {
            &Window::X(ref window) => RawWindowHandle::Xlib(window.raw_window_handle()),
            &Window::Wayland(ref window) => RawWindowHandle::Wayland(window.raw_window_handle()),
        }
    }
}

unsafe extern "C" fn x_error_callback(
    display: *mut x11::ffi::Display,
    event: *mut x11::ffi::XErrorEvent,
) -> c_int {
    let xconn_lock = X11_BACKEND.lock();
    if let Ok(ref xconn) = *xconn_lock {
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

        error!("X11 error: {:#?}", error);

        *xconn.latest_error.lock() = Some(error);
    }
    // Fun fact: this return value is completely ignored.
    0
}

pub enum EventLoop<T: 'static> {
    Wayland(wayland::EventLoop<T>),
    X(x11::EventLoop<T>),
}

pub enum EventLoopProxy<T: 'static> {
    X(x11::EventLoopProxy<T>),
    Wayland(wayland::EventLoopProxy<T>),
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        match self {
            EventLoopProxy::X(proxy) => EventLoopProxy::X(proxy.clone()),
            EventLoopProxy::Wayland(proxy) => EventLoopProxy::Wayland(proxy.clone()),
        }
    }
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        assert_is_main_thread("new_any_thread");

        EventLoop::new_any_thread()
    }

    pub fn new_any_thread() -> EventLoop<T> {
        if let Ok(env_var) = env::var(BACKEND_PREFERENCE_ENV_VAR) {
            match env_var.as_str() {
                "x11" => {
                    // TODO: propagate
                    return EventLoop::new_x11_any_thread()
                        .expect("Failed to initialize X11 backend");
                }
                "wayland" => {
                    return EventLoop::new_wayland_any_thread()
                        .expect("Failed to initialize Wayland backend");
                }
                _ => panic!(
                    "Unknown environment variable value for {}, try one of `x11`,`wayland`",
                    BACKEND_PREFERENCE_ENV_VAR,
                ),
            }
        }

        let wayland_err = match EventLoop::new_wayland_any_thread() {
            Ok(event_loop) => return event_loop,
            Err(err) => err,
        };

        let x11_err = match EventLoop::new_x11_any_thread() {
            Ok(event_loop) => return event_loop,
            Err(err) => err,
        };

        let err_string = format!(
            "Failed to initialize any backend! Wayland status: {:?} X11 status: {:?}",
            wayland_err, x11_err,
        );
        panic!(err_string);
    }

    pub fn new_wayland() -> Result<EventLoop<T>, ConnectError> {
        assert_is_main_thread("new_wayland_any_thread");

        EventLoop::new_wayland_any_thread()
    }

    pub fn new_wayland_any_thread() -> Result<EventLoop<T>, ConnectError> {
        wayland::EventLoop::new().map(EventLoop::Wayland)
    }

    pub fn new_x11() -> Result<EventLoop<T>, XNotSupported> {
        assert_is_main_thread("new_x11_any_thread");

        EventLoop::new_x11_any_thread()
    }

    pub fn new_x11_any_thread() -> Result<EventLoop<T>, XNotSupported> {
        let xconn = match X11_BACKEND.lock().as_ref() {
            Ok(xconn) => xconn.clone(),
            Err(err) => return Err(err.clone()),
        };

        Ok(EventLoop::X(x11::EventLoop::new(xconn)))
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        match *self {
            EventLoop::Wayland(ref evlp) => evlp
                .available_monitors()
                .into_iter()
                .map(MonitorHandle::Wayland)
                .collect(),
            EventLoop::X(ref evlp) => evlp
                .x_connection()
                .available_monitors()
                .into_iter()
                .map(MonitorHandle::X)
                .collect(),
        }
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        match *self {
            EventLoop::Wayland(ref evlp) => MonitorHandle::Wayland(evlp.primary_monitor()),
            EventLoop::X(ref evlp) => MonitorHandle::X(evlp.x_connection().primary_monitor()),
        }
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        match *self {
            EventLoop::Wayland(ref evlp) => EventLoopProxy::Wayland(evlp.create_proxy()),
            EventLoop::X(ref evlp) => EventLoopProxy::X(evlp.create_proxy()),
        }
    }

    pub fn run_return<F>(&mut self, callback: F)
    where
        F: FnMut(crate::event::Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        match *self {
            EventLoop::Wayland(ref mut evlp) => evlp.run_return(callback),
            EventLoop::X(ref mut evlp) => evlp.run_return(callback),
        }
    }

    pub fn run<F>(self, callback: F) -> !
    where
        F: 'static + FnMut(crate::event::Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        match self {
            EventLoop::Wayland(evlp) => evlp.run(callback),
            EventLoop::X(evlp) => evlp.run(callback),
        }
    }

    pub fn window_target(&self) -> &crate::event_loop::EventLoopWindowTarget<T> {
        match *self {
            EventLoop::Wayland(ref evl) => evl.window_target(),
            EventLoop::X(ref evl) => evl.window_target(),
        }
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        match *self {
            EventLoopProxy::Wayland(ref proxy) => proxy.send_event(event),
            EventLoopProxy::X(ref proxy) => proxy.send_event(event),
        }
    }
}

pub enum EventLoopWindowTarget<T> {
    Wayland(wayland::EventLoopWindowTarget<T>),
    X(x11::EventLoopWindowTarget<T>),
}

impl<T> EventLoopWindowTarget<T> {
    #[inline]
    pub fn is_wayland(&self) -> bool {
        match *self {
            EventLoopWindowTarget::Wayland(_) => true,
            EventLoopWindowTarget::X(_) => false,
        }
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
    // make ControlFlow::Exit sticky by providing a dummy
    // control flow reference if it is already Exit.
    let mut dummy = ControlFlow::Exit;
    let cf = if *control_flow == ControlFlow::Exit {
        &mut dummy
    } else {
        control_flow
    };
    // user callback
    callback(evt, target, cf)
}

fn assert_is_main_thread(suggested_method: &str) {
    if !is_main_thread() {
        panic!(
            "Initializing the event loop outside of the main thread is a significant \
             cross-platform compatibility hazard. If you really, absolutely need to create an \
             EventLoop on a different thread, please use the `EventLoopExtUnix::{}` function.",
            suggested_method
        );
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
    use libc::_lwp_self;

    unsafe { _lwp_self() == 1 }
}
