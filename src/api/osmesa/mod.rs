#![cfg(all(any(target_os = "linux", target_os = "freebsd"), feature="headless"))]

extern crate osmesa_sys;

use Api;
use BuilderAttribs;
use CreationError;
use CreationError::OsError;
use GlContext;
use PixelFormat;
use libc;
use std::{mem, ptr};
use std::ffi::CString;

pub struct OsMesaContext {
    context: osmesa_sys::OSMesaContext,
    buffer: Vec<u32>,
    width: u32,
    height: u32,
}

impl OsMesaContext {
    pub fn new(builder: BuilderAttribs) -> Result<OsMesaContext, CreationError> {
        let dimensions = builder.dimensions.unwrap();

        Ok(OsMesaContext {
            width: dimensions.0,
            height: dimensions.1,
            buffer: ::std::iter::repeat(unsafe { mem::uninitialized() })
                .take((dimensions.0 * dimensions.1) as usize).collect(),
            context: unsafe {
                let ctxt = osmesa_sys::OSMesaCreateContext(0x1908, ptr::null_mut());
                if ctxt.is_null() {
                    return Err(OsError("OSMesaCreateContext failed".to_string()));
                }
                ctxt
            }
        })
    }

    pub fn get_framebuffer(&self) -> &[u32] {
        &self.buffer
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    // TODO: can we remove this without causing havoc?
    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }
}

impl GlContext for OsMesaContext {
    unsafe fn make_current(&self) {
        let ret = osmesa_sys::OSMesaMakeCurrent(self.context,
            self.buffer.as_ptr() as *mut libc::c_void,
            0x1401, self.width as libc::c_int, self.height as libc::c_int);

        if ret == 0 {
            panic!("OSMesaMakeCurrent failed")
        }
    }

    fn is_current(&self) -> bool {
        unsafe { osmesa_sys::OSMesaGetCurrentContext() == self.context }
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        unsafe {
            let c_str = CString::new(addr.as_bytes().to_vec()).unwrap();
            mem::transmute(osmesa_sys::OSMesaGetProcAddress(mem::transmute(c_str.as_ptr())))
        }
    }

    fn swap_buffers(&self) {
    }

    fn get_api(&self) -> Api {
        Api::OpenGl
    }

    fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!();
    }
}

impl Drop for OsMesaContext {
    fn drop(&mut self) {
        unsafe { osmesa_sys::OSMesaDestroyContext(self.context) }
    }
}

unsafe impl Send for OsMesaContext {}
unsafe impl Sync for OsMesaContext {}
