pub use self::event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget};
pub use self::window::{DeviceId, MonitorHandle, Window, WindowId, PlatformSpecificWindowBuilderAttributes};

use std::fmt::{Display, Error, Formatter};

#[macro_use]
mod wasm_util;
mod event_loop;
mod event;
pub mod window;

#[derive(Debug)]
pub struct OsError;

impl Display for OsError {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), Error> {
        formatter.pad(&format!("websys error"))
    }
}