use crate::dpi::{PhysicalPosition, PhysicalSize};

pub mod event_loop;
pub mod window;
use crate::{monitor, platform_impl};
pub use drm::SystemError;
use drm::{
    control::{Device as ControlDevice, *},
    Device,
};
pub use event_loop::EventLoop;
pub use event_loop::EventLoopProxy;
pub use event_loop::EventLoopWindowTarget;
use std::os::unix;
use std::os::unix::prelude::FromRawFd;
use std::sync::Arc;
pub use window::Window;

#[derive(Debug, Clone)]
/// A simple wrapper for a device node.
pub struct Card(pub(crate) Arc<i32>);

/// Implementing `AsRawFd` is a prerequisite to implementing the traits found
/// in this crate. Here, we are just calling `as_raw_fd()` on the inner File.
impl unix::io::AsRawFd for Card {
    fn as_raw_fd(&self) -> unix::io::RawFd {
        *self.0
    }
}

impl Drop for Card {
    fn drop(&mut self) {
        unsafe { std::fs::File::from_raw_fd(*self.0) };
    }
}

/// With `AsRawFd` implemented, we can now implement `drm::Device`.
impl Device for Card {}
impl ControlDevice for Card {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

impl DeviceId {
    pub const unsafe fn dummy() -> Self {
        DeviceId
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorHandle {
    connector: connector::Info,
    name: String,
}

impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(
            self.connector
                .interface_id()
                .cmp(&other.connector.interface_id()),
        )
    }
}

impl Ord for MonitorHandle {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.connector
            .interface_id()
            .cmp(&other.connector.interface_id())
    }
}

impl MonitorHandle {
    #[inline]
    pub fn name(&self) -> Option<String> {
        Some(self.name.to_string())
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        self.connector.interface_id()
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        let size = self.connector.modes()[0].size();
        PhysicalSize::new(size.0 as u32, size.1 as u32)
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        PhysicalPosition::new(0, 0)
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        1.0
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = monitor::VideoMode> {
        let modes = self.connector.modes().to_vec();
        let monitor = self.connector.clone();
        modes.into_iter().map(move |f| monitor::VideoMode {
            video_mode: platform_impl::VideoMode::Kms(VideoMode {
                mode: f,
                connector: monitor.clone(),
            }),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoMode {
    mode: Mode,
    connector: connector::Info,
}

impl VideoMode {
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        let size = self.mode.size();
        PhysicalSize::new(size.0 as u32, size.1 as u32)
    }

    #[inline]
    pub fn bit_depth(&self) -> u16 {
        32
    }

    #[inline]
    pub fn refresh_rate(&self) -> u16 {
        self.mode.vrefresh() as u16
    }

    #[inline]
    pub fn monitor(&self) -> monitor::MonitorHandle {
        monitor::MonitorHandle {
            inner: platform_impl::MonitorHandle::Kms(MonitorHandle {
                connector: self.connector.clone(),
                name: self.mode.name().to_string_lossy().into_owned(),
            }),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId;

impl WindowId {
    pub const unsafe fn dummy() -> Self {
        Self
    }
}
