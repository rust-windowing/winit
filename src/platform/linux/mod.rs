#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

use std::collections::VecDeque;
use std::sync::Arc;
use std::env;

use {CreationError, CursorState, EventsLoopClosed, MouseCursor, ControlFlow};
use libc;

use self::x11::XConnection;
use self::x11::XError;
use self::x11::XNotSupported;
use self::x11::ffi::XVisualInfo;

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
}

pub enum UnixBackend {
    X(Arc<XConnection>),
    Wayland(Arc<wayland::WaylandContext>),
    Error(Option<XNotSupported>, Option<String>),
}

lazy_static!(
    pub static ref UNIX_BACKEND: UnixBackend = {
        #[inline]
        fn x_backend() -> Result<UnixBackend, XNotSupported> {
            match XConnection::new(Some(x_error_callback)) {
                Ok(x) => Ok(UnixBackend::X(Arc::new(x))),
                Err(e) => Err(e),
            }
        }
        #[inline]
        fn wayland_backend() -> Result<UnixBackend, ()> {
             wayland::WaylandContext::init()
                .map(|ctx| UnixBackend::Wayland(Arc::new(ctx)))
                .ok_or(())
        }
        match env::var(BACKEND_PREFERENCE_ENV_VAR) {
            Ok(s) => match s.as_str() {
                "x11" => x_backend().unwrap_or_else(|e| UnixBackend::Error(Some(e), None)),
                "wayland" => wayland_backend().unwrap_or_else(|_| {
                    UnixBackend::Error(None, Some("Wayland not available".into()))
                }),
                _ => panic!("Unknown environment variable value for {}, try one of `x11`,`wayland`",
                            BACKEND_PREFERENCE_ENV_VAR),
            },
            Err(_) => {
                // Try wayland, fallback to X11
                wayland_backend().unwrap_or_else(|_| {
                    x_backend().unwrap_or_else(|x_err| {
                        UnixBackend::Error(Some(x_err), Some("Wayland not available".into()))
                    })
                })
            },
        }
    };
);


pub enum Window2 {
    #[doc(hidden)]
    X(x11::Window2),
    #[doc(hidden)]
    Wayland(wayland::Window)
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WindowId {
    #[doc(hidden)]
    X(x11::WindowId),
    #[doc(hidden)]
    Wayland(wayland::WindowId)
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceId {
    #[doc(hidden)]
    X(x11::DeviceId),
    #[doc(hidden)]
    Wayland(wayland::DeviceId)
}

#[derive(Clone)]
pub enum MonitorId {
    #[doc(hidden)]
    X(x11::MonitorId),
    #[doc(hidden)]
    Wayland(wayland::MonitorId),
    #[doc(hidden)]
    None,
}

#[inline]
pub fn get_available_monitors() -> VecDeque<MonitorId> {
    match *UNIX_BACKEND {
        UnixBackend::Wayland(ref ctxt) => wayland::get_available_monitors(ctxt)
                                .into_iter()
                                .map(MonitorId::Wayland)
                                .collect(),
        UnixBackend::X(ref connec) => x11::get_available_monitors(connec)
                                    .into_iter()
                                    .map(MonitorId::X)
                                    .collect(),
        UnixBackend::Error(..) => { let mut d = VecDeque::new(); d.push_back(MonitorId::None); d},
    }
}

#[inline]
pub fn get_primary_monitor() -> MonitorId {
    match *UNIX_BACKEND {
        UnixBackend::Wayland(ref ctxt) => MonitorId::Wayland(wayland::get_primary_monitor(ctxt)),
        UnixBackend::X(ref connec) => MonitorId::X(x11::get_primary_monitor(connec)),
        UnixBackend::Error(..) => MonitorId::None,
    }
}

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        match self {
            &MonitorId::X(ref m) => m.get_name(),
            &MonitorId::Wayland(ref m) => m.get_name(),
            &MonitorId::None => None,
        }
    }

    #[inline]
    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        match self {
            &MonitorId::X(ref m) => m.get_native_identifier(),
            &MonitorId::Wayland(ref m) => m.get_native_identifier(),
            &MonitorId::None => unimplemented!()        // FIXME:
        }
    }

    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        match self {
            &MonitorId::X(ref m) => m.get_dimensions(),
            &MonitorId::Wayland(ref m) => m.get_dimensions(),
            &MonitorId::None => (800, 600),     // FIXME:
        }
    }
}

impl Window2 {
    #[inline]
    pub fn new(events_loop: &EventsLoop,
               window: &::WindowAttributes,
               pl_attribs: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Self, CreationError>
    {
        match *UNIX_BACKEND {
            UnixBackend::Wayland(ref ctxt) => {
                if let EventsLoop::Wayland(ref evlp) = *events_loop {
                    wayland::Window::new(evlp, ctxt.clone(), window).map(Window2::Wayland)
                } else {
                    // It is not possible to instanciate an EventsLoop not matching its backend
                    unreachable!()
                }
            },

            UnixBackend::X(_) => {
                x11::Window2::new(events_loop, window, pl_attribs).map(Window2::X)
            },
            UnixBackend::Error(..) => {
                // If the Backend is Error(), it is not possible to instanciate an EventsLoop at all,
                // thus this function cannot be called!
                unreachable!()
            }
        }
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        match self {
            &Window2::X(ref w) => WindowId::X(w.id()),
            &Window2::Wayland(ref w) => WindowId::Wayland(w.id())
        }
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        match self {
            &Window2::X(ref w) => w.set_title(title),
            &Window2::Wayland(ref w) => w.set_title(title)
        }
    }

    #[inline]
    pub fn show(&self) {
        match self {
            &Window2::X(ref w) => w.show(),
            &Window2::Wayland(ref w) => w.show()
        }
    }

    #[inline]
    pub fn hide(&self) {
        match self {
            &Window2::X(ref w) => w.hide(),
            &Window2::Wayland(ref w) => w.hide()
        }
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        match self {
            &Window2::X(ref w) => w.get_position(),
            &Window2::Wayland(ref w) => w.get_position()
        }
    }

    #[inline]
    pub fn set_position(&self, x: i32, y: i32) {
        match self {
            &Window2::X(ref w) => w.set_position(x, y),
            &Window2::Wayland(ref w) => w.set_position(x, y)
        }
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        match self {
            &Window2::X(ref w) => w.get_inner_size(),
            &Window2::Wayland(ref w) => w.get_inner_size()
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        match self {
            &Window2::X(ref w) => w.get_outer_size(),
            &Window2::Wayland(ref w) => w.get_outer_size()
        }
    }

    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        match self {
            &Window2::X(ref w) => w.set_inner_size(x, y),
            &Window2::Wayland(ref w) => w.set_inner_size(x, y)
        }
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        match self {
            &Window2::X(ref w) => w.set_cursor(cursor),
            &Window2::Wayland(ref w) => w.set_cursor(cursor)
        }
    }

    #[inline]
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        match self {
            &Window2::X(ref w) => w.set_cursor_state(state),
            &Window2::Wayland(ref w) => w.set_cursor_state(state)
        }
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
       match self {
            &Window2::X(ref w) => w.hidpi_factor(),
            &Window2::Wayland(ref w) => w.hidpi_factor()
        }
    }

    #[inline]
    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        match self {
            &Window2::X(ref w) => w.set_cursor_position(x, y),
            &Window2::Wayland(ref w) => w.set_cursor_position(x, y)
        }
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        use wayland_client::Proxy;
        match self {
            &Window2::X(ref w) => w.platform_display(),
            &Window2::Wayland(ref w) => w.get_display().ptr() as *mut _
        }
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        use wayland_client::Proxy;
        match self {
            &Window2::X(ref w) => w.platform_window(),
            &Window2::Wayland(ref w) => w.get_surface().ptr() as *mut _
        }
    }
}

unsafe extern "C" fn x_error_callback(dpy: *mut x11::ffi::Display, event: *mut x11::ffi::XErrorEvent)
                                      -> libc::c_int
{
    use std::ffi::CStr;

    if let UnixBackend::X(ref x) = *UNIX_BACKEND {
        let mut buff: Vec<u8> = Vec::with_capacity(1024);
        (x.xlib.XGetErrorText)(dpy, (*event).error_code as i32, buff.as_mut_ptr() as *mut libc::c_char, buff.capacity() as i32);
        let description = CStr::from_ptr(buff.as_mut_ptr() as *const libc::c_char).to_string_lossy();

        let error = XError {
            description: description.into_owned(),
            error_code: (*event).error_code,
            request_code: (*event).request_code,
            minor_code: (*event).minor_code,
        };

        *x.latest_error.lock().unwrap() = Some(error);
    }

    0
}

pub enum EventsLoop {
    #[doc(hidden)]
    Wayland(wayland::EventsLoop),
    #[doc(hidden)]
    X(x11::EventsLoop)
}

pub enum EventsLoopProxy {
    X(x11::EventsLoopProxy),
    Wayland(wayland::EventsLoopProxy),
}

impl EventsLoop {
    pub fn new() -> EventsLoop {
        match *UNIX_BACKEND {
            UnixBackend::Wayland(ref ctxt) => {
                EventsLoop::Wayland(wayland::EventsLoop::new(ctxt.clone()))
            },

            UnixBackend::X(ref ctxt) => {
                EventsLoop::X(x11::EventsLoop::new(ctxt.clone()))
            },

            UnixBackend::Error(..) => {
                panic!("Attempted to create an EventsLoop while no backend was available.")
            }
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
}

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        match *self {
            EventsLoopProxy::Wayland(ref proxy) => proxy.wakeup(),
            EventsLoopProxy::X(ref proxy) => proxy.wakeup(),
        }
    }
}
