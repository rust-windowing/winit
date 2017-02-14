#![cfg(target_os = "macos")]

pub use self::events_loop::EventsLoop;
pub use self::monitor::{MonitorId, get_available_monitors, get_primary_monitor};
pub use self::window::{Id as WindowId, PlatformSpecificWindowBuilderAttributes, Window};

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

impl Window2 {

    pub fn new(events_loop: ::std::sync::Arc<EventsLoop>,
               attributes: &::WindowAttributes,
               pl_attribs: &PlatformSpecificWindowBuilderAttributes) -> Result<Self, CreationError>
    {
        let weak_events_loop = ::std::sync::Arc::downgrade(&events_loop);
        let window = ::std::sync::Arc::new(try!(Window::new(weak_events_loop, attributes, pl_attribs)));
        events_loop.windows.lock().unwrap().push(window.clone());
        Ok(Window2 { window: window })
    }

}

mod events_loop;
mod monitor;
mod window;
