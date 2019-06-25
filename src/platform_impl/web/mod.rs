// TODO: dpi
// TODO: close events (stdweb PR required)
// TODO: pointer locking (stdweb PR required)
// TODO: mouse wheel events (stdweb PR required)
// TODO: key event: .which() (stdweb PR)
// TODO: should there be a maximization / fullscreen API?

mod device;
mod error;
mod event_loop;
mod monitor;
mod window;

#[cfg(feature = "web_sys")]
#[path = "web_sys/mod.rs"]
mod backend;

pub use self::device::Id as DeviceId;
pub use self::error::OsError;
pub use self::event_loop::{
    EventLoop, Proxy as EventLoopProxy, WindowTarget as EventLoopWindowTarget,
};
pub use self::monitor::Handle as MonitorHandle;
pub use self::window::{
    Id as WindowId, PlatformSpecificBuilderAttributes as PlatformSpecificWindowBuilderAttributes,
    Window,
};
