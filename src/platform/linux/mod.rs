#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

use Api;
use ContextError;
use CreationError;
use GlAttributes;
use GlContext;
use PixelFormat;
use PixelFormatRequirements;

use api::osmesa::{self, OsMesaContext};

pub use self::api_dispatch::{Window, WindowProxy, MonitorId, get_available_monitors, get_primary_monitor};
pub use self::api_dispatch::{WaitEventsIterator, PollEventsIterator};
pub use self::api_dispatch::PlatformSpecificWindowBuilderAttributes;
mod api_dispatch;

#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

pub struct HeadlessContext(OsMesaContext);

impl HeadlessContext {
    pub fn new(dimensions: (u32, u32), pf_reqs: &PixelFormatRequirements,
               opengl: &GlAttributes<&HeadlessContext>,
               _: &PlatformSpecificHeadlessBuilderAttributes)
               -> Result<HeadlessContext, CreationError>
    {
        let opengl = opengl.clone().map_sharing(|c| &c.0);

        match OsMesaContext::new(dimensions, pf_reqs, &opengl) {
            Ok(c) => return Ok(HeadlessContext(c)),
            Err(osmesa::OsMesaCreationError::NotSupported) => (),
            Err(osmesa::OsMesaCreationError::CreationError(e)) => return Err(e),
        };

        Err(CreationError::NotSupported)
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
