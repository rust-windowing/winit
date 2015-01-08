use BuilderAttribs;
use CreationError;
use CreationError::OsError;
use libc;
use std::{mem, ptr};
use super::ffi;

fn with_c_str<F, T>(s: &str, f: F) -> T where F: FnOnce(*const i8) -> T {
    use std::ffi::CString;
    let c_str = CString::from_slice(s.as_bytes());
    f(c_str.as_slice_with_nul().as_ptr())    
}

pub struct HeadlessContext {
    context: ffi::OSMesaContext,
    buffer: Vec<u32>,
    width: uint,
    height: uint,
}

impl HeadlessContext {
    pub fn new(builder: BuilderAttribs) -> Result<HeadlessContext, CreationError> {
        let dimensions = builder.dimensions.unwrap();

        Ok(HeadlessContext {
            width: dimensions.0,
            height: dimensions.1,
            buffer: ::std::iter::repeat(unsafe { mem::uninitialized() }).take(dimensions.0 * dimensions.1).collect(),
            context: unsafe {
                let ctxt = ffi::OSMesaCreateContext(0x1908, ptr::null());
                if ctxt.is_null() {
                    return Err(OsError("OSMesaCreateContext failed".to_string()));
                }
                ctxt
            }
        })
    }

    pub unsafe fn make_current(&self) {
        let ret = ffi::OSMesaMakeCurrent(self.context,
            self.buffer.as_ptr() as *mut libc::c_void,
            0x1401, self.width as libc::c_int, self.height as libc::c_int);

        if ret == 0 {
            panic!("OSMesaMakeCurrent failed")
        }
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        unsafe {
            with_c_str(addr, |s| {
                ffi::OSMesaGetProcAddress(mem::transmute(s)) as *const ()
            })
        }
    }

    /// See the docs in the crate root file.
    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGl
    }

    pub fn set_window_resize_callback(&mut self, _: Option<fn(uint, uint)>) {
    }
}

impl Drop for HeadlessContext {
    fn drop(&mut self) {
        unsafe { ffi::OSMesaDestroyContext(self.context) }
    }
}

unsafe impl Send for HeadlessContext {}
unsafe impl Sync for HeadlessContext {}
