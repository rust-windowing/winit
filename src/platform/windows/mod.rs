#![cfg(target_os = "windows")]

pub use api::win32;
pub use api::win32::{MonitorID, get_available_monitors, get_primary_monitor};
pub use api::win32::{WindowProxy, PollEventsIterator, WaitEventsIterator};

use libc;

use Api;
use BuilderAttribs;
use ContextError;
use CreationError;
use PixelFormat;
use GlContext;

use api::egl::ffi::egl::Egl;

use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use kernel32;

/// Stupid wrapper because `*const libc::c_void` doesn't implement `Sync`.
struct EglWrapper(Egl);
unsafe impl Sync for EglWrapper {}

lazy_static! {
    // An EGL implementation available on the system.
    static ref EGL: Option<EglWrapper> = {
        // the ATI drivers provide an EGL implementation in their DLLs
        let dll_name = if cfg!(target_pointer_width = "64") {
            b"atio6axx.dll\0"
        } else {
            b"atioglxx.dll\0"
        };

        let dll = unsafe { kernel32::LoadLibraryA(dll_name.as_ptr() as *const _) };

        if !dll.is_null() {
            let egl = Egl::load_with(|name| {
                let name = CString::new(name).unwrap();
                unsafe { kernel32::GetProcAddress(dll, name.as_ptr()) as *const _ }
            });

            Some(EglWrapper(egl))

        } else {
            None
        }
    };
}


/// The Win32 implementation of the main `Window` object.
pub struct Window(win32::Window);

impl Window {
    /// See the docs in the crate root file.
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        win32::Window::new(builder, EGL.as_ref().map(|w| &w.0)).map(|w| Window(w))
    }
}

impl Deref for Window {
    type Target = win32::Window;

    fn deref(&self) -> &win32::Window {
        &self.0
    }
}

impl DerefMut for Window {
    fn deref_mut(&mut self) -> &mut win32::Window {
        &mut self.0
    }
}

///
pub struct HeadlessContext(win32::Window);

impl HeadlessContext {
    pub fn new(mut builder: BuilderAttribs) -> Result<HeadlessContext, CreationError> {
        builder.visible = false;
        win32::Window::new(builder, EGL.as_ref().map(|w| &w.0)).map(|w| HeadlessContext(w))
    }
}

impl GlContext for HeadlessContext {
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.0.make_current()
    }

    fn is_current(&self) -> bool {
        self.0.is_current()
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        self.0.get_proc_address(addr)
    }

    fn swap_buffers(&self) -> Result<(), ContextError> {
        self.0.swap_buffers()
    }

    fn get_api(&self) -> Api {
        self.0.get_api()
    }

    fn get_pixel_format(&self) -> PixelFormat {
        self.0.get_pixel_format()
    }
}
