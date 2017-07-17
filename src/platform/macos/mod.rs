#![cfg(target_os = "macos")]

pub use self::events_loop::{EventsLoop, Proxy as EventsLoopProxy};
pub use self::monitor::{MonitorId, get_available_monitors, get_primary_monitor};
pub use self::window::{Id as WindowId, PlatformSpecificWindowBuilderAttributes, Window};
use std::sync::Arc;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

use {CreationError};

pub struct Window2 {
    pub window: Arc<Window>,
}

impl ::std::ops::Deref for Window2 {
    type Target = Window;
    #[inline]
    fn deref(&self) -> &Window {
        &*self.window
    }
}

impl Window2 {

    pub fn new(events_loop: &EventsLoop,
               attributes: &::WindowAttributes,
               pl_attribs: &PlatformSpecificWindowBuilderAttributes) -> Result<Self, CreationError>
    {
        let weak_shared = Arc::downgrade(&events_loop.shared);
        let window = Arc::new(try!(Window::new(weak_shared, attributes, pl_attribs)));
        let weak_window = Arc::downgrade(&window);
        events_loop.shared.windows.lock().unwrap().push(weak_window);
        Ok(Window2 { window: window })
    }

}

mod events_loop;
mod monitor;
mod window;

mod timer;

#[cfg(not(feature="context"))]
mod send_event;

#[cfg(feature="context")]
mod send_event_context;
#[cfg(feature="context")]
use self::send_event_context as send_event;
