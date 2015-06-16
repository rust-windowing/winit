#![cfg(target_os = "emscripten")]

use std::ffi::CString;
use libc;
use {Event, BuilderAttribs, CreationError, MouseCursor};
use Api;
use PixelFormat;
use GlContext;

use std::collections::VecDeque;

mod ffi;

pub struct Window {
    context: ffi::EMSCRIPTEN_WEBGL_CONTEXT_HANDLE,
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        None
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        None
    }
}

#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        unimplemented!()
    }
}

pub struct MonitorID;

pub fn get_available_monitors() -> VecDeque<MonitorID> {
    let mut list = VecDeque::new();
    list.push_back(MonitorID);
    list
}

pub fn get_primary_monitor() -> MonitorID {
    MonitorID
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        Some("Canvas".to_string())
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!()
    }
}

impl Window {
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        // getting the default values of attributes
        let mut attributes = unsafe {
            use std::mem;
            let mut attributes: ffi::EmscriptenWebGLContextAttributes = mem::uninitialized();
            ffi::emscripten_webgl_init_context_attributes(&mut attributes);
            attributes
        };

        // setting the attributes
        // FIXME: 
        /*match builder.gl_version {
            Some((major, minor)) => {
                attributes.majorVersion = major as libc::c_int;
                attributes.minorVersion = minor as libc::c_int;
            },
            None => ()
        };*/

        // creating the context
        let context = unsafe {
            use std::{mem, ptr};
            let context = ffi::emscripten_webgl_create_context(ptr::null(), &attributes);
            if context <= 0 {
                return Err(CreationError::OsError(format!("Error while calling emscripten_webgl_create_context: {}",
                    error_to_str(mem::transmute(context)))));
            }
            context
        };

        // TODO: emscripten_set_webglcontextrestored_callback

        Ok(Window {
            context: context
        })
    }

    pub fn set_title(&self, _title: &str) {
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        Some((0, 0))
    }

    pub fn set_position(&self, _: i32, _: i32) {
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unsafe {
            use std::{mem, ptr};
            let mut width = mem::uninitialized();
            let mut height = mem::uninitialized();

            if ffi::emscripten_get_element_css_size(ptr::null(), &mut width, &mut height)
                != ffi::EMSCRIPTEN_RESULT_SUCCESS
            {
                None
            } else {
                Some((width as u32, height as u32))
            }
        }
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.get_inner_size()
    }

    pub fn set_inner_size(&self, width: u32, height: u32) {
        unsafe {
            use std::ptr;
            ffi::emscripten_set_element_css_size(ptr::null(), width as libc::c_double, height
                as libc::c_double);
        }
    }

    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self,
        }
    }

    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self,
        }
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    pub fn show(&self) {}
    pub fn hide(&self) {}

    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    pub fn set_cursor(&self, _cursor: MouseCursor) {
        unimplemented!()
    }

    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }
}

impl GlContext for Window {
    unsafe fn make_current(&self) {
        // TOOD: check if == EMSCRIPTEN_RESULT
        ffi::emscripten_webgl_make_context_current(self.context);
    }

    fn is_current(&self) -> bool {
        true        // FIXME: 
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();

        unsafe {
            ffi::emscripten_GetProcAddress(addr) as *const _
        }
    }

    fn swap_buffers(&self) {
        unsafe {
            ffi::emscripten_sleep(1);   // FIXME: 
        }
    }

    fn get_api(&self) -> Api {
        Api::WebGl
    }

    fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!();
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            ffi::emscripten_exit_fullscreen();
            ffi::emscripten_webgl_destroy_context(self.context);
        }
    }
}

fn error_to_str(code: ffi::EMSCRIPTEN_RESULT) -> &'static str {
    match code {
        ffi::EMSCRIPTEN_RESULT_SUCCESS | ffi::EMSCRIPTEN_RESULT_DEFERRED
            => "Internal error in the library (success detected as failure)",

        ffi::EMSCRIPTEN_RESULT_NOT_SUPPORTED => "Not supported",
        ffi::EMSCRIPTEN_RESULT_FAILED_NOT_DEFERRED => "Failed not deferred",
        ffi::EMSCRIPTEN_RESULT_INVALID_TARGET => "Invalid target",
        ffi::EMSCRIPTEN_RESULT_UNKNOWN_TARGET => "Unknown target",
        ffi::EMSCRIPTEN_RESULT_INVALID_PARAM => "Invalid parameter",
        ffi::EMSCRIPTEN_RESULT_FAILED => "Failed",
        ffi::EMSCRIPTEN_RESULT_NO_DATA => "No data",

        _ => "Undocumented error"
    }
}
