#![cfg(target_os = "macos")]

pub use self::events_loop::EventsLoop;
pub use self::monitor::{MonitorId, get_available_monitors, get_primary_monitor};
pub use self::window::{Id as WindowId, PlatformSpecificWindowBuilderAttributes, Window};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

use {CreationError};

pub struct Window2 {
    pub window: ::std::sync::Arc<Window>,
}

impl ::std::ops::Deref for Window2 {
    type Target = Window;
    #[inline]
    fn deref(&self) -> &Window {
        &*self.window
    }
}

use libc::c_void;

impl Window2 {

    pub fn with_handle(events_loop: ::std::sync::Arc<EventsLoop>, handle: *mut c_void) -> Result<Self, CreationError> {

        let weak_events_loop = ::std::sync::Arc::downgrade(&events_loop);

        let window = ::std::sync::Arc::new(try!(Window::with_handle(weak_events_loop, handle)));
        
        let weak_window = ::std::sync::Arc::downgrade(&window);
        events_loop.windows.lock().unwrap().push(weak_window);
        Ok(Window2 { window: window })
    }

    pub fn new(events_loop: ::std::sync::Arc<EventsLoop>,
               attributes: &::WindowAttributes,
               pl_attribs: &PlatformSpecificWindowBuilderAttributes) -> Result<Self, CreationError>
    {
        let weak_events_loop = ::std::sync::Arc::downgrade(&events_loop);
        let window = ::std::sync::Arc::new(try!(Window::new(weak_events_loop, attributes, pl_attribs)));
        let weak_window = ::std::sync::Arc::downgrade(&window);
        events_loop.windows.lock().unwrap().push(weak_window);
        Ok(Window2 { window: window })
    }

}

mod events_loop;
mod monitor;
mod window;
