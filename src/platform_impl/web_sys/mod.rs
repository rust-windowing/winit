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

#[derive(Debug)]
pub struct OsError(String);

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn window() -> web_sys::Window {
    web_sys::window().unwrap()
}

fn document() -> web_sys::Document {
    window().document().unwrap()
}
