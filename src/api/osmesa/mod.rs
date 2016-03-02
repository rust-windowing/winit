#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

extern crate osmesa_sys;

use Api;
use ContextError;
use CreationError;
use GlAttributes;
use GlContext;
use PixelFormat;
use PixelFormatRequirements;
use Robustness;
use libc;
use std::{mem, ptr};
use std::ffi::CString;

pub struct OsMesaContext {
    context: osmesa_sys::OSMesaContext,
    buffer: Vec<u32>,
    width: u32,
    height: u32,
}

pub enum OsMesaCreationError {
    CreationError(CreationError),
    NotSupported,
}

impl From<CreationError> for OsMesaCreationError {
    #[inline]
    fn from(e: CreationError) -> OsMesaCreationError {
        OsMesaCreationError::CreationError(e)
    }
}

impl OsMesaContext {
    pub fn new(dimensions: (u32, u32), pf_reqs: &PixelFormatRequirements,
               opengl: &GlAttributes<&OsMesaContext>) -> Result<OsMesaContext, OsMesaCreationError>
    {
        if let Err(_) = osmesa_sys::OsMesa::try_loading() {
            return Err(OsMesaCreationError::NotSupported);
        }

        if opengl.sharing.is_some() { unimplemented!() }        // TODO: proper error

        match opengl.robustness {
            Robustness::RobustNoResetNotification | Robustness::RobustLoseContextOnReset => {
                return Err(CreationError::RobustnessNotSupported.into());
            },
            _ => ()
        }

        // TODO: use `pf_reqs` for the format
        // TODO: check OpenGL version and return `OpenGlVersionNotSupported` if necessary

        Ok(OsMesaContext {
            width: dimensions.0,
            height: dimensions.1,
            buffer: ::std::iter::repeat(unsafe { mem::uninitialized() })
                .take((dimensions.0 * dimensions.1) as usize).collect(),
            context: unsafe {
                let ctxt = osmesa_sys::OSMesaCreateContext(0x1908, ptr::null_mut());
                if ctxt.is_null() {
                    return Err(CreationError::OsError("OSMesaCreateContext failed".to_string()).into());
                }
                ctxt
            }
        })
    }

    #[inline]
    pub fn get_framebuffer(&self) -> &[u32] {
        &self.buffer
    }

    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[allow(dead_code)]
    // TODO: can we remove this without causing havoc?
    #[inline]
    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }
}

impl GlContext for OsMesaContext {
    #[inline]
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        let ret = osmesa_sys::OSMesaMakeCurrent(self.context, self.buffer.as_ptr()
                                                as *mut _, 0x1401, self.width
                                                as libc::c_int, self.height as libc::c_int);

        // an error can only happen in case of invalid parameter, which would indicate a bug
        // in glutin
        if ret == 0 {
            panic!("OSMesaMakeCurrent failed");
        }

        Ok(())
    }

    #[inline]
    fn is_current(&self) -> bool {
        unsafe { osmesa_sys::OSMesaGetCurrentContext() == self.context }
    }

    fn get_proc_address(&self, addr: &str) -> *const () {
        unsafe {
            let c_str = CString::new(addr.as_bytes().to_vec()).unwrap();
            mem::transmute(osmesa_sys::OSMesaGetProcAddress(mem::transmute(c_str.as_ptr())))
        }
    }

    #[inline]
    fn swap_buffers(&self) -> Result<(), ContextError> {
        Ok(())
    }

    #[inline]
    fn get_api(&self) -> Api {
        Api::OpenGl
    }

    #[inline]
    fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!();
    }
}

impl Drop for OsMesaContext {
    #[inline]
    fn drop(&mut self) {
        unsafe { osmesa_sys::OSMesaDestroyContext(self.context) }
    }
}

unsafe impl Send for OsMesaContext {}
unsafe impl Sync for OsMesaContext {}
