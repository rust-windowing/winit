#![cfg(all(any(target_os = "linux", target_os = "freebsd"), feature="headless"))]

use libc;
use api::osmesa::OsMesaContext;

use BuilderAttribs;
use CreationError;
use Event;
use PixelFormat;
use CursorState;
use MouseCursor;

use std::collections::VecDeque;
use std::ptr;

mod libcaca;

pub struct Window {
    libcaca: libcaca::LibCaca,
    display: *mut libcaca::caca_display_t,
    opengl: OsMesaContext,
    dither: *mut libcaca::caca_dither_t,
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
    VecDeque::new()
}
pub fn get_primary_monitor() -> MonitorID {
    MonitorID
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        unimplemented!();
    }

    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        ::native_monitor::NativeMonitorId::Unavailable
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!();
    }
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
        loop {}
    }
}

impl Window {
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        let opengl = try!(OsMesaContext::new(builder));
        let opengl_dimensions = opengl.get_dimensions();

        let libcaca = match libcaca::LibCaca::open() {
            Err(_) => return Err(CreationError::NotSupported),
            Ok(l) => l
        };

        let display = unsafe { libcaca.caca_create_display(ptr::null_mut()) };

        if display.is_null() {
            return Err(CreationError::OsError("caca_create_display failed".to_string()));
        }

        let dither = unsafe {
            #[cfg(target_endian = "little")]
            fn get_masks() -> (u32, u32, u32, u32) { (0xff, 0xff00, 0xff0000, 0xff000000) }
            #[cfg(target_endian = "big")]
            fn get_masks() -> (u32, u32, u32, u32) { (0xff000000, 0xff0000, 0xff00, 0xff) }

            let masks = get_masks();
            libcaca.caca_create_dither(32, opengl_dimensions.0 as libc::c_int,
                                       opengl_dimensions.1 as libc::c_int,
                                       opengl_dimensions.0 as libc::c_int * 4,
                                       masks.0, masks.1, masks.2, masks.3)
        };

        if dither.is_null() {
            unsafe { libcaca.caca_free_display(display) };
            return Err(CreationError::OsError("caca_create_dither failed".to_string()));
        }

        Ok(Window {
            libcaca: libcaca,
            display: display,
            opengl: opengl,
            dither: dither,
        })
    }

    pub fn is_closed(&self) -> bool {
        false
    }

    pub fn set_title(&self, title: &str) {
    }

    pub fn show(&self) {
    }

    pub fn hide(&self) {
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        unimplemented!()
    }

    pub fn set_position(&self, x: i32, y: i32) {
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        Some(self.opengl.get_dimensions())
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.get_inner_size()
    }

    pub fn set_inner_size(&self, _x: u32, _y: u32) {
        unimplemented!()
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        unimplemented!()
    }

    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self
        }
    }

    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self
        }
    }

    pub unsafe fn make_current(&self) {
        self.opengl.make_current()
    }

    pub fn is_current(&self) -> bool {
        self.opengl.is_current()
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        self.opengl.get_proc_address(addr) as *const _
    }

    pub fn swap_buffers(&self) {
        unsafe {
            let canvas = self.libcaca.caca_get_canvas(self.display);
            let width = self.libcaca.caca_get_canvas_width(canvas);
            let height = self.libcaca.caca_get_canvas_height(canvas);

            let buffer = self.opengl.get_framebuffer().chunks(self.opengl.get_dimensions().0 as usize)
                                    .flat_map(|i| i.iter().cloned()).rev().collect::<Vec<u32>>();

            self.libcaca.caca_dither_bitmap(canvas, 0, 0, width as libc::c_int,
                                            height as libc::c_int, self.dither,
                                            buffer.as_ptr() as *const _);
            self.libcaca.caca_refresh_display(self.display);
        };
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn get_api(&self) -> ::Api {
        self.opengl.get_api()
    }

    pub fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!();
    }

    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        Ok(())
    }

    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        Ok(())
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            self.libcaca.caca_free_dither(self.dither);
            self.libcaca.caca_free_display(self.display);
        }
    }
}
