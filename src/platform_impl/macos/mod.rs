#![cfg(target_os = "macos")]

mod app;
mod app_delegate;
mod app_state;
mod event;
mod event_loop;
mod ffi;
mod monitor;
mod observer;
mod util;
mod view;
mod window;
mod window_delegate;

use std::{ops::Deref, sync::Arc};

use {
    event::DeviceId as RootDeviceId, window::{CreationError, WindowAttributes},
};
pub use self::{
    event_loop::{EventLoop, EventLoopWindowTarget, Proxy as EventLoopProxy},
    monitor::MonitorHandle,
    window::{
        Id as WindowId, PlatformSpecificWindowBuilderAttributes, UnownedWindow,
    },
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        DeviceId
    }
}

// Constant device ID; to be removed when if backend is updated to report real device IDs.
pub(crate) const DEVICE_ID: RootDeviceId = RootDeviceId(DeviceId);

pub struct Window {
    window: Arc<UnownedWindow>,
    // We keep this around so that it doesn't get dropped until the window does.
    _delegate: util::IdRef,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Deref for Window {
    type Target = UnownedWindow;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &*self.window
    }
}

impl Window {
    pub fn new<T: 'static>(
        _window_target: &EventLoopWindowTarget<T>,
        attributes: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, CreationError> {
        let (window, _delegate) = UnownedWindow::new(attributes, pl_attribs)?;
        Ok(Window { window, _delegate })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OsSpecificWindowEvent;
