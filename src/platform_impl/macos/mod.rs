#![deny(unsafe_op_in_unsafe_fn)]

#[macro_use]
mod util;

mod app;
mod app_delegate;
mod app_state;
mod appkit;
mod event;
mod event_loop;
mod ffi;
mod menu;
mod monitor;
mod observer;
mod view;
mod window;
mod window_delegate;

use std::{fmt, ops::Deref};

use self::window::WinitWindow;
use self::window_delegate::WinitWindowDelegate;
pub(crate) use self::{
    event_loop::{
        EventLoop, EventLoopProxy, EventLoopWindowTarget, PlatformSpecificEventLoopAttributes,
    },
    monitor::{MonitorHandle, VideoMode},
    window::{PlatformSpecificWindowBuilderAttributes, WindowId},
};
use crate::{
    error::OsError as RootOsError, event::DeviceId as RootDeviceId, window::WindowAttributes,
};
use objc2::rc::{autoreleasepool, Id, Shared};

pub(crate) use crate::icon::NoIcon as PlatformIcon;
pub(self) use crate::platform_impl::Fullscreen;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

impl DeviceId {
    pub const unsafe fn dummy() -> Self {
        DeviceId
    }
}

// Constant device ID; to be removed when if backend is updated to report real device IDs.
pub(crate) const DEVICE_ID: RootDeviceId = RootDeviceId(DeviceId);

pub(crate) struct Window {
    pub(crate) window: Id<WinitWindow, Shared>,
    // We keep this around so that it doesn't get dropped until the window does.
    _delegate: Id<WinitWindowDelegate, Shared>,
}

impl Drop for Window {
    fn drop(&mut self) {
        // Ensure the window is closed
        util::close_sync(&self.window);
    }
}

#[derive(Debug)]
pub enum OsError {
    CGError(core_graphics::base::CGError),
    CreationError(&'static str),
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Deref for Window {
    type Target = WinitWindow;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

impl Window {
    pub(crate) fn new<T: 'static>(
        _window_target: &EventLoopWindowTarget<T>,
        attributes: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, RootOsError> {
        let (window, _delegate) = autoreleasepool(|_| WinitWindow::new(attributes, pl_attribs))?;
        Ok(Window { window, _delegate })
    }
}

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OsError::CGError(e) => f.pad(&format!("CGError {e}")),
            OsError::CreationError(e) => f.pad(e),
        }
    }
}
