#![cfg(target_os = "windows")]

pub use api::win32::*;

use Api;
use BuilderAttribs;
use CreationError;

///
pub struct HeadlessContext(Window);

impl HeadlessContext {
    pub fn new(mut builder: BuilderAttribs) -> Result<HeadlessContext, CreationError> {
        builder.visible = false;
        Window::new(builder).map(|w| HeadlessContext(w))
    }

    pub unsafe fn make_current(&self) {
        self.0.make_current()
    }

    pub fn is_current(&self) -> bool {
        self.0.is_current()
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        self.0.get_proc_address(addr)
    }

    pub fn get_api(&self) -> Api {
        self.0.get_api()
    }
}
