#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]
#![allow(unused_variables, dead_code)]

use libc;
use api::osmesa::{OsMesaContext, OsMesaCreationError};

use Api;
use ContextError;
use CreationError;
use Event;
use GlAttributes;
use GlContext;
use PixelFormat;
use PixelFormatRequirements;
use CursorState;
use MouseCursor;
use WindowAttributes;

use std::collections::VecDeque;
use std::path::Path;
use std::ptr;

mod ffi;

pub struct Window {
    libcaca: ffi::LibCaca,
    display: *mut ffi::caca_display_t,
    opengl: OsMesaContext,
    dither: *mut ffi::caca_dither_t,
}

#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        unimplemented!()
    }
}

pub struct MonitorId;

#[inline]
pub fn get_available_monitors() -> VecDeque<MonitorId> {
    VecDeque::new()
}
#[inline]
pub fn get_primary_monitor() -> MonitorId {
    MonitorId
}

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        unimplemented!();
    }

    #[inline]
    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        ::native_monitor::NativeMonitorId::Unavailable
    }

    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!();
    }
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    #[inline]
    fn next(&mut self) -> Option<Event> {
        None
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    #[inline]
    fn next(&mut self) -> Option<Event> {
        loop {}
    }
}

impl Window {
    pub fn new(window: &WindowAttributes, pf_reqs: &PixelFormatRequirements,
               opengl: &GlAttributes<&Window>) -> Result<Window, CreationError>
    {
        let opengl = opengl.clone().map_sharing(|w| &w.opengl);

        let opengl = match OsMesaContext::new(window.dimensions.unwrap_or((800, 600)), pf_reqs,
                                              &opengl)
        {
            Err(OsMesaCreationError::NotSupported) => return Err(CreationError::NotSupported),
            Err(OsMesaCreationError::CreationError(e)) => return Err(e),
            Ok(c) => c
        };

        let opengl_dimensions = opengl.get_dimensions();

        let libcaca = match ffi::LibCaca::open(&Path::new("libcaca.so.0")) {
            Err(_) => return Err(CreationError::NotSupported),
            Ok(l) => l
        };

        let display = unsafe { (libcaca.caca_create_display)(ptr::null_mut()) };

        if display.is_null() {
            return Err(CreationError::OsError("caca_create_display failed".to_string()));
        }

        let dither = unsafe {
            #[cfg(target_endian = "little")]
            fn get_masks() -> (u32, u32, u32, u32) { (0xff, 0xff00, 0xff0000, 0xff000000) }
            #[cfg(target_endian = "big")]
            fn get_masks() -> (u32, u32, u32, u32) { (0xff000000, 0xff0000, 0xff00, 0xff) }

            let masks = get_masks();
            (libcaca.caca_create_dither)(32, opengl_dimensions.0 as libc::c_int,
                                         opengl_dimensions.1 as libc::c_int,
                                         opengl_dimensions.0 as libc::c_int * 4,
                                         masks.0, masks.1, masks.2, masks.3)
        };

        if dither.is_null() {
            unsafe { (libcaca.caca_free_display)(display) };
            return Err(CreationError::OsError("caca_create_dither failed".to_string()));
        }

        Ok(Window {
            libcaca: libcaca,
            display: display,
            opengl: opengl,
            dither: dither,
        })
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
    }

    #[inline]
    pub fn show(&self) {
    }

    #[inline]
    pub fn hide(&self) {
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        unimplemented!()
    }

    #[inline]
    pub fn set_position(&self, x: i32, y: i32) {
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        Some(self.opengl.get_dimensions())
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.get_inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, _x: u32, _y: u32) {
        unimplemented!()
    }

    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        unimplemented!()
    }

    #[inline]
    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self
        }
    }

    #[inline]
    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self
        }
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!();
    }

    #[inline]
    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
    }

    #[inline]
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        Ok(())
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    #[inline]
    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        Ok(())
    }
}

impl GlContext for Window {
    #[inline]
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.opengl.make_current()
    }

    #[inline]
    fn is_current(&self) -> bool {
        self.opengl.is_current()
    }

    #[inline]
    fn get_proc_address(&self, addr: &str) -> *const () {
        self.opengl.get_proc_address(addr)
    }

    #[inline]
    fn swap_buffers(&self) -> Result<(), ContextError> {
        unsafe {
            let canvas = (self.libcaca.caca_get_canvas)(self.display);
            let width = (self.libcaca.caca_get_canvas_width)(canvas);
            let height = (self.libcaca.caca_get_canvas_height)(canvas);

            let buffer = self.opengl.get_framebuffer().chunks(self.opengl.get_dimensions().0 as usize)
                                    .flat_map(|i| i.iter().cloned()).rev().collect::<Vec<u32>>();

            (self.libcaca.caca_dither_bitmap)(canvas, 0, 0, width as libc::c_int,
                                              height as libc::c_int, self.dither,
                                              buffer.as_ptr() as *const _);
            (self.libcaca.caca_refresh_display)(self.display);
        };

        Ok(())
    }

    #[inline]
    fn get_api(&self) -> Api {
        self.opengl.get_api()
    }

    #[inline]
    fn get_pixel_format(&self) -> PixelFormat {
        self.opengl.get_pixel_format()
    }
}

impl Drop for Window {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            (self.libcaca.caca_free_dither)(self.dither);
            (self.libcaca.caca_free_display)(self.display);
        }
    }
}
