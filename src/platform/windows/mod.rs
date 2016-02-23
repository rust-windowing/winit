#![cfg(target_os = "windows")]

pub use api::win32;
pub use api::win32::{MonitorId, get_available_monitors, get_primary_monitor};
pub use api::win32::{WindowProxy, PollEventsIterator, WaitEventsIterator};

use CreationError;
use WindowAttributes;

use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use kernel32;

#[derive(Default)]
pub struct PlatformSpecificWindowBuilderAttributes;
#[derive(Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

/// The Win32 implementation of the main `Window` object.
pub struct Window(win32::Window);

impl Window {
    /// See the docs in the crate root file.
    #[inline]
    pub fn new(window: &WindowAttributes, _: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError>
    {
        win32::Window::new(window).map(Window)
    }
}

impl Deref for Window {
    type Target = win32::Window;

    #[inline]
    fn deref(&self) -> &win32::Window {
        &self.0
    }
}

impl DerefMut for Window {
    #[inline]
    fn deref_mut(&mut self) -> &mut win32::Window {
        &mut self.0
    }
}
