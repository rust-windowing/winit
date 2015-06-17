/*#[cfg(feature = "window")]
pub use api::x11::{Window, WindowProxy, MonitorID, get_available_monitors, get_primary_monitor};
#[cfg(feature = "window")]
pub use api::x11::{WaitEventsIterator, PollEventsIterator};*/

use std::collections::VecDeque;
use std::sync::Arc;

use BuilderAttribs;
use ContextError;
use CreationError;
use CursorState;
use Event;
use GlContext;
use MouseCursor;
use PixelFormat;
use libc;

use api::wayland;
use api::x11;
use api::x11::XConnection;

enum Backend {
    X(Arc<XConnection>),
    Wayland
}

lazy_static!(
    static ref BACKEND: Backend = {
        // Wayland backend is not production-ready yet so we disable it
        if false && wayland::is_available() {
            Backend::Wayland
        } else {
            Backend::X(Arc::new(XConnection::new().unwrap()))
        }
    };
);

pub enum Window {
    #[doc(hidden)]
    X(x11::Window),
    #[doc(hidden)]
    Wayland(wayland::Window)
}

#[derive(Clone)]
pub enum WindowProxy {
    #[doc(hidden)]
    X(x11::WindowProxy),
    #[doc(hidden)]
    Wayland(wayland::WindowProxy)
}

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        match self {
            &WindowProxy::X(ref wp) => wp.wakeup_event_loop(),
            &WindowProxy::Wayland(ref wp) => wp.wakeup_event_loop()
        }
    }
}

pub enum MonitorID {
    #[doc(hidden)]
    X(x11::MonitorID),
    #[doc(hidden)]
    Wayland(wayland::MonitorID)
}

pub fn get_available_monitors() -> VecDeque<MonitorID> {
    match *BACKEND {
        Backend::Wayland => wayland::get_available_monitors()
                                .into_iter()
                                .map(MonitorID::Wayland)
                                .collect(),
        Backend::X(ref connec) => x11::get_available_monitors(connec)
                                    .into_iter()
                                    .map(MonitorID::X)
                                    .collect(),
    }
}

pub fn get_primary_monitor() -> MonitorID {
    match *BACKEND {
        Backend::Wayland => MonitorID::Wayland(wayland::get_primary_monitor()),
        Backend::X(ref connec) => MonitorID::X(x11::get_primary_monitor(connec)),
    }
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        match self {
            &MonitorID::X(ref m) => m.get_name(),
            &MonitorID::Wayland(ref m) => m.get_name()
        }
    }

    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        match self {
            &MonitorID::X(ref m) => m.get_native_identifier(),
            &MonitorID::Wayland(ref m) => m.get_native_identifier()
        }
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        match self {
            &MonitorID::X(ref m) => m.get_dimensions(),
            &MonitorID::Wayland(ref m) => m.get_dimensions()
        }
    }
}


pub enum PollEventsIterator<'a> {
    #[doc(hidden)]
    X(x11::PollEventsIterator<'a>),
    #[doc(hidden)]
    Wayland(wayland::PollEventsIterator<'a>)
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        match self {
            &mut PollEventsIterator::X(ref mut it) => it.next(),
            &mut PollEventsIterator::Wayland(ref mut it) => it.next()
        }
    }
}

pub enum WaitEventsIterator<'a> {
    #[doc(hidden)]
    X(x11::WaitEventsIterator<'a>),
    #[doc(hidden)]
    Wayland(wayland::WaitEventsIterator<'a>)
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        match self {
            &mut WaitEventsIterator::X(ref mut it) => it.next(),
            &mut WaitEventsIterator::Wayland(ref mut it) => it.next()
        }
    }
}

impl Window {
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        match *BACKEND {
            Backend::Wayland => wayland::Window::new(builder).map(Window::Wayland),
            Backend::X(ref connec) => x11::Window::new(connec, builder).map(Window::X),
        }
    }

    pub fn set_title(&self, title: &str) {
        match self {
            &Window::X(ref w) => w.set_title(title),
            &Window::Wayland(ref w) => w.set_title(title)
        }
    }

    pub fn show(&self) {
        match self {
            &Window::X(ref w) => w.show(),
            &Window::Wayland(ref w) => w.show()
        }
    }

    pub fn hide(&self) {
        match self {
            &Window::X(ref w) => w.hide(),
            &Window::Wayland(ref w) => w.hide()
        }
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        match self {
            &Window::X(ref w) => w.get_position(),
            &Window::Wayland(ref w) => w.get_position()
        }
    }

    pub fn set_position(&self, x: i32, y: i32) {
        match self {
            &Window::X(ref w) => w.set_position(x, y),
            &Window::Wayland(ref w) => w.set_position(x, y)
        }
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        match self {
            &Window::X(ref w) => w.get_inner_size(),
            &Window::Wayland(ref w) => w.get_inner_size()
        }
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        match self {
            &Window::X(ref w) => w.get_outer_size(),
            &Window::Wayland(ref w) => w.get_outer_size()
        }
    }

    pub fn set_inner_size(&self, x: u32, y: u32) {
        match self {
            &Window::X(ref w) => w.set_inner_size(x, y),
            &Window::Wayland(ref w) => w.set_inner_size(x, y)
        }
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        match self {
            &Window::X(ref w) => WindowProxy::X(w.create_window_proxy()),
            &Window::Wayland(ref w) => WindowProxy::Wayland(w.create_window_proxy())
        }
    }

    pub fn poll_events(&self) -> PollEventsIterator {
        match self {
            &Window::X(ref w) => PollEventsIterator::X(w.poll_events()),
            &Window::Wayland(ref w) => PollEventsIterator::Wayland(w.poll_events())
        }
    }

    pub fn wait_events(&self) -> WaitEventsIterator {
        match self {
            &Window::X(ref w) => WaitEventsIterator::X(w.wait_events()),
            &Window::Wayland(ref w) => WaitEventsIterator::Wayland(w.wait_events())
        }
    }

    pub fn set_window_resize_callback(&mut self, callback: Option<fn(u32, u32)>) {
        match self {
            &mut Window::X(ref mut w) => w.set_window_resize_callback(callback),
            &mut Window::Wayland(ref mut w) => w.set_window_resize_callback(callback)
        }
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        match self {
            &Window::X(ref w) => w.set_cursor(cursor),
            &Window::Wayland(ref w) => w.set_cursor(cursor)
        }
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        match self {
            &Window::X(ref w) => w.set_cursor_state(state),
            &Window::Wayland(ref w) => w.set_cursor_state(state)
        }
    }

    pub fn hidpi_factor(&self) -> f32 {
       match self {
            &Window::X(ref w) => w.hidpi_factor(),
            &Window::Wayland(ref w) => w.hidpi_factor()
        }
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        match self {
            &Window::X(ref w) => w.set_cursor_position(x, y),
            &Window::Wayland(ref w) => w.set_cursor_position(x, y)
        }
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        match self {
            &Window::X(ref w) => w.platform_display(),
            &Window::Wayland(ref w) => w.platform_display()
        }
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        match self {
            &Window::X(ref w) => w.platform_window(),
            &Window::Wayland(ref w) => w.platform_window()
        }
    }
}

impl GlContext for Window {
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        match self {
            &Window::X(ref w) => w.make_current(),
            &Window::Wayland(ref w) => w.make_current()
        }
    }

    fn is_current(&self) -> bool {
        match self {
            &Window::X(ref w) => w.is_current(),
            &Window::Wayland(ref w) => w.is_current()
        }
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        match self {
            &Window::X(ref w) => w.get_proc_address(addr),
            &Window::Wayland(ref w) => w.get_proc_address(addr)
        }
    }

    fn swap_buffers(&self) -> Result<(), ContextError> {
        match self {
            &Window::X(ref w) => w.swap_buffers(),
            &Window::Wayland(ref w) => w.swap_buffers()
        }
    }

    fn get_api(&self) -> ::Api {
        match self {
            &Window::X(ref w) => w.get_api(),
            &Window::Wayland(ref w) => w.get_api()
        }
    }

    fn get_pixel_format(&self) -> PixelFormat {
        match self {
            &Window::X(ref w) => w.get_pixel_format(),
            &Window::Wayland(ref w) => w.get_pixel_format()
        }
    }
}
