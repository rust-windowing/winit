use libc;
use std::ffi::CStr;
use std::mem;
use api::dlopen;

pub type caca_display_t = libc::c_void;
pub type caca_canvas_t = libc::c_void;
pub type caca_dither_t = libc::c_void;

pub struct LibCaca {
    lib: *mut libc::c_void,

    caca_create_display: unsafe extern fn(*mut caca_canvas_t) -> *mut caca_display_t,
    caca_free_display: unsafe extern fn(*mut caca_display_t) -> libc::c_int,
    caca_get_canvas: unsafe extern fn(*mut caca_display_t) -> *mut caca_canvas_t,
    caca_refresh_display: unsafe extern fn(*mut caca_display_t) -> libc::c_int,
    caca_dither_bitmap: unsafe extern fn(*mut caca_canvas_t, libc::c_int, libc::c_int, libc::c_int,
                                         libc::c_int, *const caca_dither_t, *const libc::c_void)
                                         -> libc::c_int,
    caca_free_dither: unsafe extern fn(*mut caca_dither_t) -> libc::c_int,
    caca_create_dither: unsafe extern fn(libc::c_int, libc::c_int, libc::c_int, libc::c_int,
                                         libc::uint32_t, libc::uint32_t, libc::uint32_t,
                                         libc::uint32_t) -> *mut caca_dither_t,
    caca_get_canvas_width: unsafe extern fn(*mut caca_canvas_t) -> libc::c_int,
    caca_get_canvas_height: unsafe extern fn(*mut caca_canvas_t) -> libc::c_int,
}

#[derive(Debug)]
pub struct OpenError {
    reason: String
}

impl LibCaca {
    pub fn open() -> Result<LibCaca, OpenError> {
        let lib = unsafe { dlopen::dlopen(b"libcaca.so.0\0".as_ptr() as *const _,
                                          dlopen::RTLD_NOW) };

        if lib.is_null() {
            let cstr = unsafe { CStr::from_ptr(dlopen::dlerror()) };
            let reason = String::from_utf8(cstr.to_bytes().to_vec()).unwrap();
            return Err(OpenError { reason: reason });
        }

        let caca_create_display = match unsafe { dlopen::dlsym(lib,
                                        b"caca_create_display\0".as_ptr() as *const _) }
        {
            ptr if ptr.is_null() => return Err(OpenError {
                reason: "Could not load caca_create_display".to_string()
            }),
            ptr => ptr
        };

        let caca_free_display = match unsafe { dlopen::dlsym(lib,
                                      b"caca_free_display\0".as_ptr() as *const _) }
        {
            ptr if ptr.is_null() => return Err(OpenError {
                reason: "Could not load caca_free_display".to_string()
            }),
            ptr => ptr
        };

        let caca_get_canvas = match unsafe { dlopen::dlsym(lib,
                                    b"caca_get_canvas\0".as_ptr() as *const _) }
        {
            ptr if ptr.is_null() => return Err(OpenError {
                reason: "Could not load caca_get_canvas".to_string()
            }),
            ptr => ptr
        };

        let caca_refresh_display = match unsafe { dlopen::dlsym(lib,
                                         b"caca_refresh_display\0".as_ptr() as *const _) }
        {
            ptr if ptr.is_null() => return Err(OpenError {
                reason: "Could not load caca_refresh_display".to_string()
            }),
            ptr => ptr
        };

        let caca_dither_bitmap = match unsafe { dlopen::dlsym(lib,
                                       b"caca_dither_bitmap\0".as_ptr() as *const _) }
        {
            ptr if ptr.is_null() => return Err(OpenError {
                reason: "Could not load caca_dither_bitmap".to_string()
            }),
            ptr => ptr
        };

        let caca_free_dither = match unsafe { dlopen::dlsym(lib,
                                     b"caca_free_dither\0".as_ptr() as *const _) }
        {
            ptr if ptr.is_null() => return Err(OpenError {
                reason: "Could not load caca_free_dither".to_string()
            }),
            ptr => ptr
        };

        let caca_create_dither = match unsafe { dlopen::dlsym(lib,
                                       b"caca_create_dither\0".as_ptr() as *const _) }
        {
            ptr if ptr.is_null() => return Err(OpenError {
                reason: "Could not load caca_create_dither".to_string()
            }),
            ptr => ptr
        };

        let caca_get_canvas_width = match unsafe { dlopen::dlsym(lib,
                                          b"caca_get_canvas_width\0".as_ptr() as *const _) }
        {
            ptr if ptr.is_null() => return Err(OpenError {
                reason: "Could not load caca_get_canvas_width".to_string()
            }),
            ptr => ptr
        };

        let caca_get_canvas_height = match unsafe { dlopen::dlsym(lib,
                                           b"caca_get_canvas_height\0".as_ptr() as *const _) }
        {
            ptr if ptr.is_null() => return Err(OpenError {
                reason: "Could not load caca_get_canvas_height".to_string()
            }),
            ptr => ptr
        };

        Ok(LibCaca {
            lib: lib,

            caca_create_display: unsafe { mem::transmute(caca_create_display) },
            caca_free_display: unsafe { mem::transmute(caca_free_display) },
            caca_get_canvas: unsafe { mem::transmute(caca_get_canvas) },
            caca_refresh_display: unsafe { mem::transmute(caca_refresh_display) },
            caca_dither_bitmap: unsafe { mem::transmute(caca_dither_bitmap) },
            caca_free_dither: unsafe { mem::transmute(caca_free_dither) },
            caca_create_dither: unsafe { mem::transmute(caca_create_dither) },
            caca_get_canvas_width: unsafe { mem::transmute(caca_get_canvas_width) },
            caca_get_canvas_height: unsafe { mem::transmute(caca_get_canvas_height) },
        })
    }

    pub unsafe fn caca_create_display(&self, cv: *mut caca_canvas_t) -> *mut caca_display_t {
        (self.caca_create_display)(cv)
    }

    pub unsafe fn caca_free_display(&self, dp: *mut caca_display_t) -> libc::c_int {
        (self.caca_free_display)(dp)
    }

    pub unsafe fn caca_get_canvas(&self, dp: *mut caca_display_t) -> *mut caca_canvas_t {
        (self.caca_get_canvas)(dp)
    }

    pub unsafe fn caca_refresh_display(&self, dp: *mut caca_display_t) -> libc::c_int {
        (self.caca_refresh_display)(dp)
    }

    pub unsafe fn caca_dither_bitmap(&self, cv: *mut caca_canvas_t, x: libc::c_int, y: libc::c_int,
                                     w: libc::c_int, h: libc::c_int, d: *const caca_dither_t,
                                     pixels: *const libc::c_void) -> libc::c_int
    {
        (self.caca_dither_bitmap)(cv, x, y, w, h, d, pixels)
    }

    pub unsafe fn caca_free_dither(&self, d: *mut caca_dither_t) -> libc::c_int {
        (self.caca_free_dither)(d)
    }

    pub unsafe fn caca_create_dither(&self, bpp: libc::c_int, w: libc::c_int, h: libc::c_int,
                                     pitch: libc::c_int, rmask: libc::uint32_t, gmask: libc::uint32_t,
                                     bmask: libc::uint32_t, amask: libc::uint32_t) -> *mut caca_dither_t
    {
        (self.caca_create_dither)(bpp, w, h, pitch, rmask, gmask, bmask, amask)
    }

    pub unsafe fn caca_get_canvas_width(&self, cv: *mut caca_canvas_t) -> libc::c_int {
        (self.caca_get_canvas_width)(cv)
    }

    pub unsafe fn caca_get_canvas_height(&self, cv: *mut caca_canvas_t) -> libc::c_int {
        (self.caca_get_canvas_height)(cv)
    }
}

impl Drop for LibCaca {
    fn drop(&mut self) {
        unsafe { dlopen::dlclose(self.lib); }
    }
}
