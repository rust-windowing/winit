#![cfg(target_os = "macos")]

mod app;
mod app_delegate;
mod app_state;
mod event;
mod event_loop;
mod ffi;
mod menu;
mod monitor;
mod observer;
mod util;
mod view;
mod window;
mod window_delegate;

use std::{fmt, ops::Deref, sync::Arc};

pub use self::{
    app_delegate::{get_aux_state_mut, AuxDelegateState},
    event_loop::{EventLoop, EventLoopWindowTarget, Proxy as EventLoopProxy},
    monitor::{MonitorHandle, VideoMode},
    window::{Id as WindowId, PlatformSpecificWindowBuilderAttributes, UnownedWindow},
};
use crate::{
    error::OsError as RootOsError, event::DeviceId as RootDeviceId, window::WindowAttributes,
};
use objc::rc::autoreleasepool;

pub(crate) use crate::icon::NoIcon as PlatformIcon;

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

#[derive(Debug)]
pub enum OsError {
    CGError(core_graphics::base::CGError),
    CreationError(&'static str),
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
    ) -> Result<Self, RootOsError> {
        let (window, _delegate) = autoreleasepool(|| UnownedWindow::new(attributes, pl_attribs))?;
        Ok(Window { window, _delegate })
    }
}

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OsError::CGError(e) => f.pad(&format!("CGError {}", e)),
            OsError::CreationError(e) => f.pad(e),
        }
    }
}
