#![cfg(target_os = "android")]

pub use api::android::*;

#[cfg(feature = "headless")]
pub struct HeadlessContext(i32);

#[cfg(feature = "headless")]
impl HeadlessContext {
    /// See the docs in the crate root file.
    pub fn new(_builder: BuilderAttribs) -> Result<HeadlessContext, CreationError> {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub unsafe fn make_current(&self) {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub fn is_current(&self) -> bool {
        unimplemented!()
    }

    /// See the docs in the crate root file.
    pub fn get_proc_address(&self, _addr: &str) -> *const () {
        unimplemented!()
    }

    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGlEs
    }
}

#[cfg(feature = "headless")]
unsafe impl Send for HeadlessContext {}
#[cfg(feature = "headless")]
unsafe impl Sync for HeadlessContext {}
