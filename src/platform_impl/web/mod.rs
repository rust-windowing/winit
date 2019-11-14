// TODO: close events (port from old stdweb branch)
// TODO: pointer locking (stdweb PR required)
// TODO: fullscreen API (stdweb PR required)

mod device;
mod error;
mod event_loop;
mod monitor;
mod window;

#[cfg(feature = "web-sys")]
#[path = "web_sys/mod.rs"]
mod backend;

#[cfg(feature = "stdweb")]
#[path = "stdweb/mod.rs"]
mod backend;

#[cfg(not(any(feature = "web-sys", feature = "stdweb")))]
compile_error!("Please select a feature to build for web: `web-sys`, `stdweb`");

pub use self::device::Id as DeviceId;
pub use self::error::OsError;
pub use self::event_loop::{
    EventLoop, Proxy as EventLoopProxy, WindowTarget as EventLoopWindowTarget,
};
pub use self::monitor::{Handle as MonitorHandle, Mode as VideoMode};
pub use self::window::{
    Id as WindowId, PlatformSpecificBuilderAttributes as PlatformSpecificWindowBuilderAttributes,
    Window,
};
