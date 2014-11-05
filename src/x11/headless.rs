use HeadlessRendererBuilder;
use CreationError;
use libc;
use std::{mem, ptr};
use super::ffi;

pub struct HeadlessContext {
    context: ffi::OSMesaContext,
    buffer: Vec<u32>,
    width: uint,
    height: uint,
}

impl HeadlessContext {
    pub fn new(builder: HeadlessRendererBuilder) -> Result<HeadlessContext, CreationError> {
        Ok(HeadlessContext {
            width: builder.dimensions.0,
            height: builder.dimensions.1,
            buffer: Vec::from_elem(builder.dimensions.0 * builder.dimensions.1, unsafe { mem::uninitialized() }),
            context: unsafe {
                // TODO: check errors
                ffi::OSMesaCreateContext(0x1908, ptr::null())
            }
        })
    }

    pub unsafe fn make_current(&self) {
        ffi::OSMesaMakeCurrent(self.context,
            self.buffer.as_ptr() as *mut libc::c_void,
            0x1401, self.width as libc::c_int, self.height as libc::c_int);
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        use std::c_str::ToCStr;

        unsafe {
            addr.with_c_str(|s| {
                ffi::OSMesaGetProcAddress(mem::transmute(s)) as *const ()
            })
        }
    }
}

impl Drop for HeadlessContext {
    fn drop(&mut self) {
        unsafe { ffi::OSMesaDestroyContext(self.context) }
    }
}
