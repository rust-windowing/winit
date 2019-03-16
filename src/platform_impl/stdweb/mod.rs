mod event_loop;
mod events;
mod window;

pub use self::event_loop::{DeviceId, EventLoop, EventLoopRunnerShared, EventLoopWindowTarget, EventLoopProxy, register};
pub use self::window::{MonitorHandle, Window, WindowId, PlatformSpecificWindowBuilderAttributes};
pub use self::events::{button_mapping, mouse_modifiers_state, mouse_button, keyboard_modifiers_state, scancode};


// TODO: dpi
// TODO: close events (stdweb PR required)
// TODO: pointer locking (stdweb PR required)
// TODO: mouse wheel events (stdweb PR required)
// TODO: key event: .which() (stdweb PR)
// TODO: should there be a maximization / fullscreen API?
