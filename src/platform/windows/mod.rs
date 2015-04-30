#![cfg(target_os = "windows")]

pub use api::win32::*;

use libc;

use Api;
use BuilderAttribs;
use CreationError;
use PixelFormat;
use GlContext;

///
pub struct HeadlessContext(Window);

impl HeadlessContext {
    pub fn new(mut builder: BuilderAttribs) -> Result<HeadlessContext, CreationError> {
        builder.visible = false;
        Window::new(builder).map(|w| HeadlessContext(w))
    }
}

impl GlContext for HeadlessContext {
    unsafe fn make_current(&self) {
        self.0.make_current()
    }

    fn is_current(&self) -> bool {
        self.0.is_current()
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        self.0.get_proc_address(addr)
    }

    fn swap_buffers(&self) {
        self.0.swap_buffers()
    }

    fn get_api(&self) -> Api {
        self.0.get_api()
    }

    fn get_pixel_format(&self) -> PixelFormat {
        self.0.get_pixel_format()
    }
}
