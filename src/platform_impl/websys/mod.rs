pub use self::event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget};
pub use self::window::{DeviceId, MonitorHandle, Window, WindowId, PlatformSpecificWindowBuilderAttributes};

#[macro_use]
mod wasm_util;
mod event_loop;
mod event;
pub mod window;