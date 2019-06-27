use std::fmt;

mod event_loop;
mod event;
mod window;

pub use self::event_loop::{DeviceId, EventLoop, EventLoopRunnerShared, EventLoopWindowTarget, EventLoopProxy, register};
pub use self::window::{MonitorHandle, Window, WindowId, PlatformSpecificWindowBuilderAttributes};
pub use self::event::{button_mapping, mouse_modifiers_state, mouse_button, keyboard_modifiers_state, scancode};

#[derive(Debug)]
pub struct OsError(String);

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

