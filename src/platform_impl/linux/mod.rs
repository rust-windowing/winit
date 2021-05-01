#![cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

mod event_loop;
mod monitor;
mod window;

pub use window::{WindowId, Window, PlatformSpecificWindowBuilderAttributes, PlatformIcon};
pub use event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget};
pub use monitor::{MonitorHandle, VideoMode};

#[derive(Debug, Clone)]
pub struct OsError;

impl std::fmt::Display for OsError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(usize);

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        Self(0)
    }
}
