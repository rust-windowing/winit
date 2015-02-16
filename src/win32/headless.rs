use super::Window;
use super::init;

use Api;
use BuilderAttribs;
use CreationError;

///
pub struct HeadlessContext(Window);

impl HeadlessContext {
    /// See the docs in the crate root file.
    pub fn new(builder: BuilderAttribs) -> Result<HeadlessContext, CreationError> {
        let (builder, _) = builder.extract_non_static();
        init::new_window(builder, None).map(|w| HeadlessContext(w))
    }

    /// See the docs in the crate root file.
    pub unsafe fn make_current(&self) {
        self.0.make_current()
    }

    /// See the docs in the crate root file.
    pub fn get_proc_address(&self, addr: &str) -> *const () {
        self.0.get_proc_address(addr)
    }

    /// See the docs in the crate root file.
    pub fn get_api(&self) -> Api {
        Api::OpenGl
    }

    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }
}
