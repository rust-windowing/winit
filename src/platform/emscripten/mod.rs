#![cfg(target_os = "emscripten")]

use Api;
use ContextError;
use CreationError;
use GlAttributes;
use GlContext;
use PixelFormat;
use PixelFormatRequirements;

pub use api::emscripten::{Window, WindowProxy, MonitorId, get_available_monitors};
pub use api::emscripten::{get_primary_monitor, WaitEventsIterator, PollEventsIterator};

pub struct HeadlessContext(Window);

impl HeadlessContext {
    /// See the docs in the crate root file.
    #[inline]
    pub fn new(_: (u32, u32), _: &PixelFormatRequirements, _: &GlAttributes<&HeadlessContext>)
               -> Result<HeadlessContext, CreationError>
    {
        unimplemented!()
    }
}

impl GlContext for HeadlessContext {
    #[inline]
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.0.make_current()
    }

    #[inline]
    fn is_current(&self) -> bool {
        self.0.is_current()
    }

    #[inline]
    fn get_proc_address(&self, addr: &str) -> *const () {
        self.0.get_proc_address(addr)
    }

    #[inline]
    fn swap_buffers(&self) -> Result<(), ContextError> {
        self.0.swap_buffers()
    }

    #[inline]
    fn get_api(&self) -> Api {
        self.0.get_api()
    }

    #[inline]
    fn get_pixel_format(&self) -> PixelFormat {
        self.0.get_pixel_format()
    }
}

unsafe impl Send for HeadlessContext {}
unsafe impl Sync for HeadlessContext {}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;
#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;
