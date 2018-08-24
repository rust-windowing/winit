#![cfg(target_os = "macos")]

pub use self::event_loop::{EventLoop, Proxy as EventLoopProxy};
pub use self::monitor::MonitorId;
pub use self::window::{Id as WindowId, PlatformSpecificWindowBuilderAttributes, Window2};
use std::sync::Arc;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

use {CreationError};

pub struct Window {
    pub window: Arc<Window2>,
}

impl ::std::ops::Deref for Window {
    type Target = Window2;
    #[inline]
    fn deref(&self) -> &Window2 {
        &*self.window
    }
}

impl Window {

    pub fn new(event_loop: &EventLoop,
               attributes: ::WindowAttributes,
               pl_attribs: PlatformSpecificWindowBuilderAttributes) -> Result<Self, CreationError>
    {
        let weak_shared = Arc::downgrade(&event_loop.shared);
        let window = Arc::new(try!(Window2::new(weak_shared, attributes, pl_attribs)));
        let weak_window = Arc::downgrade(&window);
        event_loop.shared.windows.lock().unwrap().push(weak_window);
        Ok(Window { window: window })
    }

}

mod event_loop;
mod ffi;
mod monitor;
mod util;
mod view;
mod window;
