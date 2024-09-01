#![cfg(free_unix)]

#[cfg(all(not(x11_platform), not(wayland_platform)))]
compile_error!("Please select a feature to build for unix: `x11`, `wayland`");

use std::num::{NonZeroU16, NonZeroU32};
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::sync::Arc;
use std::time::Duration;
use std::{env, fmt};
#[cfg(x11_platform)]
use std::{ffi::CStr, mem::MaybeUninit, os::raw::*, sync::Mutex};

use smol_str::SmolStr;

pub(crate) use self::common::xkb::{physicalkey_to_scancode, scancode_to_physicalkey};
#[cfg(x11_platform)]
use self::x11::{X11Error, XConnection, XError, XNotSupported};
use crate::application::ApplicationHandler;
pub(crate) use crate::cursor::OnlyCursorImageSource as PlatformCustomCursorSource;
#[cfg(x11_platform)]
use crate::dpi::Size;
use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::error::EventLoopError;
use crate::event_loop::ActiveEventLoop;
pub(crate) use crate::icon::RgbaIcon as PlatformIcon;
use crate::keyboard::Key;
use crate::platform::pump_events::PumpStatus;
#[cfg(x11_platform)]
use crate::platform::x11::{WindowType as XWindowType, XlibErrorHook};
#[cfg(x11_platform)]
use crate::utils::Lazy;
use crate::window::ActivationToken;

pub(crate) mod common;
#[cfg(wayland_platform)]
pub(crate) mod wayland;
#[cfg(x11_platform)]
pub(crate) mod x11;

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

#[derive(Clone, Debug, PartialEq)]
pub struct PlatformSpecificWindowAttributes {
    pub name: Option<ApplicationName>,
    pub activation_token: Option<ActivationToken>,
    #[cfg(x11_platform)]
    pub x11: X11WindowAttributes,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg(x11_platform)]
pub struct X11WindowAttributes {
    pub visual_id: Option<x11rb::protocol::xproto::Visualid>,
    pub screen_id: Option<i32>,
    pub base_size: Option<Size>,
    pub override_redirect: bool,
    pub x11_window_types: Vec<XWindowType>,

    /// The parent window to embed this window into.
    pub embed_window: Option<x11rb::protocol::xproto::Window>,
}

#[cfg_attr(not(x11_platform), allow(clippy::derivable_impls))]
impl Default for PlatformSpecificWindowAttributes {
    fn default() -> Self {
        Self {
            name: None,
            activation_token: None,
            #[cfg(x11_platform)]
            x11: X11WindowAttributes {
                visual_id: None,
                screen_id: None,
                base_size: None,
                override_redirect: false,
                x11_window_types: vec![XWindowType::Normal],
                embed_window: None,
            },
        }
    }
}

#[cfg(x11_platform)]
pub(crate) static X11_BACKEND: Lazy<Mutex<Result<Arc<XConnection>, XNotSupported>>> =
    Lazy::new(|| Mutex::new(XConnection::new(Some(x_error_callback)).map(Arc::new)));

#[derive(Debug, Clone)]
pub enum OsError {
    Misc(&'static str),
    #[cfg(x11_platform)]
    XNotSupported(XNotSupported),
    #[cfg(x11_platform)]
    XError(Arc<X11Error>),
    #[cfg(wayland_platform)]
    WaylandError(Arc<wayland::WaylandError>),
}

impl fmt::Display for OsError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            OsError::Misc(e) => _f.pad(e),
            #[cfg(x11_platform)]
            OsError::XNotSupported(ref e) => fmt::Display::fmt(e, _f),
            #[cfg(x11_platform)]
            OsError::XError(ref e) => fmt::Display::fmt(e, _f),
            #[cfg(wayland_platform)]
            OsError::WaylandError(ref e) => fmt::Display::fmt(e, _f),
        }
    }
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
    pub const fn dummy() -> Self {
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
    pub const fn dummy() -> Self {
        #[cfg(wayland_platform)]
        return DeviceId::Wayland(wayland::DeviceId::dummy());
        #[cfg(all(not(wayland_platform), x11_platform))]
        return DeviceId::X(x11::DeviceId::dummy());
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FingerId {
    #[cfg(x11_platform)]
    X(x11::FingerId),
    #[cfg(wayland_platform)]
    Wayland(wayland::FingerId),
}

impl FingerId {
    pub const fn dummy() -> Self {
        #[cfg(wayland_platform)]
        return FingerId::Wayland(wayland::FingerId::dummy());
        #[cfg(all(not(wayland_platform), x11_platform))]
        return FingerId::X(x11::FingerId::dummy());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    pub fn position(&self) -> Option<PhysicalPosition<i32>> {
        x11_or_wayland!(match self; MonitorHandle(m) => m.position())
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        x11_or_wayland!(match self; MonitorHandle(m) => m.scale_factor() as _)
    }

    #[inline]
    pub fn current_video_mode(&self) -> Option<VideoModeHandle> {
        x11_or_wayland!(match self; MonitorHandle(m) => m.current_video_mode())
    }

    #[inline]
    pub fn video_modes(&self) -> Box<dyn Iterator<Item = VideoModeHandle>> {
        x11_or_wayland!(match self; MonitorHandle(m) => Box::new(m.video_modes()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VideoModeHandle {
    #[cfg(x11_platform)]
    X(x11::VideoModeHandle),
    #[cfg(wayland_platform)]
    Wayland(wayland::VideoModeHandle),
}

impl VideoModeHandle {
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        x11_or_wayland!(match self; VideoModeHandle(m) => m.size())
    }

    #[inline]
    pub fn bit_depth(&self) -> Option<NonZeroU16> {
        x11_or_wayland!(match self; VideoModeHandle(m) => m.bit_depth())
    }

    #[inline]
    pub fn refresh_rate_millihertz(&self) -> Option<NonZeroU32> {
        x11_or_wayland!(match self; VideoModeHandle(m) => m.refresh_rate_millihertz())
    }

    #[inline]
    pub fn monitor(&self) -> MonitorHandle {
        x11_or_wayland!(match self; VideoModeHandle(m) => m.monitor(); as MonitorHandle)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct KeyEventExtra {
    pub text_with_all_modifiers: Option<SmolStr>,
    pub key_without_modifiers: Key,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PlatformCustomCursor {
    #[cfg(wayland_platform)]
    Wayland(wayland::CustomCursor),
    #[cfg(x11_platform)]
    X(x11::CustomCursor),
}

/// Hooks for X11 errors.
#[cfg(x11_platform)]
pub(crate) static mut XLIB_ERROR_HOOKS: Mutex<Vec<XlibErrorHook>> = Mutex::new(Vec::new());

#[cfg(x11_platform)]
unsafe extern "C" fn x_error_callback(
    display: *mut x11::ffi::Display,
    event: *mut x11::ffi::XErrorEvent,
) -> c_int {
    let xconn_lock = X11_BACKEND.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(ref xconn) = *xconn_lock {
        // Call all the hooks.
        let mut error_handled = false;
        for hook in unsafe { XLIB_ERROR_HOOKS.lock() }.unwrap().iter() {
            error_handled |= hook(display as *mut _, event as *mut _);
        }

        // `assume_init` is safe here because the array consists of `MaybeUninit` values,
        // which do not require initialization.
        let mut buf: [MaybeUninit<c_char>; 1024] = unsafe { MaybeUninit::uninit().assume_init() };
        unsafe {
            (xconn.xlib.XGetErrorText)(
                display,
                (*event).error_code as c_int,
                buf.as_mut_ptr() as *mut c_char,
                buf.len() as c_int,
            )
        };
        let description =
            unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) }.to_string_lossy();

        let error = unsafe {
            XError {
                description: description.into_owned(),
                error_code: (*event).error_code,
                request_code: (*event).request_code,
                minor_code: (*event).minor_code,
            }
        };

        // Don't log error.
        if !error_handled {
            tracing::error!("X11 error: {:#?}", error);
            // XXX only update the error, if it wasn't handled by any of the hooks.
            *xconn.latest_error.lock().unwrap() = Some(error);
        }
    }
    // Fun fact: this return value is completely ignored.
    0
}

pub enum EventLoop {
    #[cfg(wayland_platform)]
    Wayland(Box<wayland::EventLoop>),
    #[cfg(x11_platform)]
    X(x11::EventLoop),
}

#[derive(Clone)]
pub enum EventLoopProxy {
    #[cfg(x11_platform)]
    X(x11::EventLoopProxy),
    #[cfg(wayland_platform)]
    Wayland(wayland::EventLoopProxy),
}

impl EventLoop {
    pub(crate) fn new(
        attributes: &PlatformSpecificEventLoopAttributes,
    ) -> Result<Self, EventLoopError> {
        if !attributes.any_thread && !is_main_thread() {
            panic!(
                "Initializing the event loop outside of the main thread is a significant \
                 cross-platform compatibility hazard. If you absolutely need to create an \
                 EventLoop on a different thread, you can use the \
                 `EventLoopBuilderExtX11::any_thread` or `EventLoopBuilderExtWayland::any_thread` \
                 functions."
            );
        }

        // NOTE: Wayland first because of X11 could be present under Wayland as well. Empty
        // variables are also treated as not set.
        let backend = match (
            attributes.forced_backend,
            env::var("WAYLAND_DISPLAY")
                .ok()
                .filter(|var| !var.is_empty())
                .or_else(|| env::var("WAYLAND_SOCKET").ok())
                .filter(|var| !var.is_empty())
                .is_some(),
            env::var("DISPLAY").map(|var| !var.is_empty()).unwrap_or(false),
        ) {
            // User is forcing a backend.
            (Some(backend), ..) => backend,
            // Wayland is present.
            #[cfg(wayland_platform)]
            (None, true, _) => Backend::Wayland,
            // X11 is present.
            #[cfg(x11_platform)]
            (None, _, true) => Backend::X,
            // No backend is present.
            (_, wayland_display, x11_display) => {
                let msg = if wayland_display && !cfg!(wayland_platform) {
                    "DISPLAY is not set; note: enable the `winit/wayland` feature to support \
                     Wayland"
                } else if x11_display && !cfg!(x11_platform) {
                    "neither WAYLAND_DISPLAY nor WAYLAND_SOCKET is set; note: enable the \
                     `winit/x11` feature to support X11"
                } else {
                    "neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set."
                };
                return Err(EventLoopError::Os(os_error!(OsError::Misc(msg))));
            },
        };

        // Create the display based on the backend.
        match backend {
            #[cfg(wayland_platform)]
            Backend::Wayland => EventLoop::new_wayland_any_thread().map_err(Into::into),
            #[cfg(x11_platform)]
            Backend::X => EventLoop::new_x11_any_thread().map_err(Into::into),
        }
    }

    #[cfg(wayland_platform)]
    fn new_wayland_any_thread() -> Result<EventLoop, EventLoopError> {
        wayland::EventLoop::new().map(|evlp| EventLoop::Wayland(Box::new(evlp)))
    }

    #[cfg(x11_platform)]
    fn new_x11_any_thread() -> Result<EventLoop, EventLoopError> {
        let xconn = match X11_BACKEND.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            Ok(xconn) => xconn.clone(),
            Err(err) => {
                return Err(EventLoopError::Os(os_error!(OsError::XNotSupported(err.clone()))))
            },
        };

        Ok(EventLoop::X(x11::EventLoop::new(xconn)))
    }

    #[inline]
    pub fn is_wayland(&self) -> bool {
        match *self {
            #[cfg(wayland_platform)]
            EventLoop::Wayland(_) => true,
            #[cfg(x11_platform)]
            _ => false,
        }
    }

    pub fn run<A: ApplicationHandler>(
        self,
        init_closure: impl FnOnce(&dyn ActiveEventLoop) -> A,
    ) -> Result<(), EventLoopError> {
        x11_or_wayland!(match self; EventLoop(evlp) => evlp.run(init_closure))
    }

    pub fn run_on_demand<A: ApplicationHandler>(
        &mut self,
        init_closure: impl FnOnce(&dyn ActiveEventLoop) -> A,
    ) -> Result<(), EventLoopError> {
        x11_or_wayland!(match self; EventLoop(evlp) => evlp.run_on_demand(init_closure))
    }

    pub fn pump_app_events<A: ApplicationHandler>(
        &mut self,
        timeout: Option<Duration>,
        app: A,
    ) -> PumpStatus {
        x11_or_wayland!(match self; EventLoop(evlp) => evlp.pump_app_events(timeout, app))
    }

    pub fn window_target(&self) -> &dyn ActiveEventLoop {
        x11_or_wayland!(match self; EventLoop(evlp) => evlp.window_target())
    }
}

impl AsFd for EventLoop {
    fn as_fd(&self) -> BorrowedFd<'_> {
        x11_or_wayland!(match self; EventLoop(evlp) => evlp.as_fd())
    }
}

impl AsRawFd for EventLoop {
    fn as_raw_fd(&self) -> RawFd {
        x11_or_wayland!(match self; EventLoop(evlp) => evlp.as_raw_fd())
    }
}

impl EventLoopProxy {
    pub fn wake_up(&self) {
        x11_or_wayland!(match self; EventLoopProxy(proxy) => proxy.wake_up())
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) enum OwnedDisplayHandle {
    #[cfg(x11_platform)]
    X(Arc<XConnection>),
    #[cfg(wayland_platform)]
    Wayland(wayland_client::Connection),
}

impl OwnedDisplayHandle {
    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        use std::ptr::NonNull;

        match self {
            #[cfg(x11_platform)]
            Self::X(xconn) => Ok(rwh_06::XlibDisplayHandle::new(
                NonNull::new(xconn.display.cast()),
                xconn.default_screen_index() as _,
            )
            .into()),

            #[cfg(wayland_platform)]
            Self::Wayland(conn) => {
                use sctk::reexports::client::Proxy;

                Ok(rwh_06::WaylandDisplayHandle::new(
                    NonNull::new(conn.display().id().as_ptr().cast()).unwrap(),
                )
                .into())
            },
        }
    }
}

impl PartialEq for OwnedDisplayHandle {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            #[cfg(x11_platform)]
            (Self::X(this), Self::X(other)) => Arc::as_ptr(this).eq(&Arc::as_ptr(other)),
            #[cfg(wayland_platform)]
            (Self::Wayland(this), Self::Wayland(other)) => this.eq(other),
            #[cfg(all(x11_platform, wayland_platform))]
            _ => false,
        }
    }
}

impl Eq for OwnedDisplayHandle {}

/// Returns the minimum `Option<Duration>`, taking into account that `None`
/// equates to an infinite timeout, not a zero timeout (so can't just use
/// `Option::min`)
fn min_timeout(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    a.map_or(b, |a_timeout| b.map_or(Some(a_timeout), |b_timeout| Some(a_timeout.min(b_timeout))))
}

#[cfg(target_os = "linux")]
fn is_main_thread() -> bool {
    rustix::thread::gettid() == rustix::process::getpid()
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
