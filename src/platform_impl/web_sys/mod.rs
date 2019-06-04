use std::fmt;

mod event_loop;
mod events;
mod window;

pub use self::event_loop::{
    register, DeviceId, EventLoop, EventLoopProxy, EventLoopRunnerShared, EventLoopWindowTarget,
};
pub use self::events::{
    button_mapping, keyboard_modifiers_state, mouse_button, mouse_modifiers_state, scancode,
};
pub use self::window::{MonitorHandle, PlatformSpecificWindowBuilderAttributes, Window, WindowId};

// TODO: unify with stdweb impl.

#[derive(Debug)]
pub struct OsError(String);

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
