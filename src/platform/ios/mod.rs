#![cfg(target_os = "ios")]
use libc::c_void;

use BuilderAttribs;
use CreationError;
use PixelFormat;

pub use api::ios::*;

pub struct HeadlessContext(i32);

impl HeadlessContext {
    /// See the docs in the crate root file.
    pub fn new(_builder: BuilderAttribs) -> Result<HeadlessContext, CreationError> {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub unsafe fn make_current(&self) {
        unimplemented!()
    }

    pub fn swap_buffers(&self) {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub fn is_current(&self) -> bool {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub fn get_proc_address(&self, _addr: &str) -> *const c_void {
        unimplemented!()
    }

    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGlEs
    }

    pub fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!();
    }
}

unsafe impl Send for HeadlessContext {}
unsafe impl Sync for HeadlessContext {}
