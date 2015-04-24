use std::marker::PhantomData;
use std::io;

use libc;
use winapi;
use CreationError;

use super::gl;
use super::ContextWrapper;
use super::WindowWrapper;

/// A guard for when you want to make the context current. Destroying the guard restores the
/// previously-current context.
pub struct CurrentContextGuard<'a, 'b> {
    previous_hdc: winapi::HDC,
    previous_hglrc: winapi::HGLRC,
    marker1: PhantomData<&'a ()>,
    marker2: PhantomData<&'b ()>,
}

impl<'a, 'b> CurrentContextGuard<'a, 'b> {
    pub unsafe fn make_current(window: &'a WindowWrapper, context: &'b ContextWrapper)
                               -> Result<CurrentContextGuard<'a, 'b>, CreationError>
    {
        let previous_hdc = gl::wgl::GetCurrentDC() as winapi::HDC;
        let previous_hglrc = gl::wgl::GetCurrentContext() as winapi::HGLRC;

        let result = gl::wgl::MakeCurrent(window.1 as *const libc::c_void,
                                          context.0 as *const libc::c_void);

        if result == 0 {
            return Err(CreationError::OsError(format!("wglMakeCurrent function failed: {}",
                                                      format!("{}", io::Error::last_os_error()))));
        }

        Ok(CurrentContextGuard {
            previous_hdc: previous_hdc,
            previous_hglrc: previous_hglrc,
            marker1: PhantomData,
            marker2: PhantomData,
        })
    }
}

impl<'a, 'b> Drop for CurrentContextGuard<'a, 'b> {
    fn drop(&mut self) {
        unsafe {
            gl::wgl::MakeCurrent(self.previous_hdc as *const libc::c_void,
                                 self.previous_hglrc as *const libc::c_void);
        }
    }
}
