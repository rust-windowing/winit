use {ContextError, CreationError, CursorState, Event, GlAttributes, GlContext,
     MouseCursor, PixelFormat, PixelFormatRequirements, WindowAttributes};

use api::egl::Context as EglContext;

use libc;

#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        unimplemented!()
    }
}

pub struct Window {
    pub context: EglContext,
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        unimplemented!()
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        unimplemented!()
    }
}

impl Window {
    pub fn new(window: &WindowAttributes, pf_reqs: &PixelFormatRequirements,
               opengl: &GlAttributes<&Window>) -> Result<Window, CreationError>
    {
        // not implemented
        assert!(window.min_dimensions.is_none());
        assert!(window.max_dimensions.is_none());

        unimplemented!()
    }

    pub fn set_title(&self, title: &str) {
        unimplemented!()
    }

    #[inline]
    pub fn show(&self) {
        unimplemented!()
    }

    #[inline]
    pub fn hide(&self) {
        unimplemented!()
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        unimplemented!()
    }

    #[inline]
    pub fn set_position(&self, _x: i32, _y: i32) {
        unimplemented!()
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unimplemented!()
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        unimplemented!()
    }

    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        unimplemented!()
    }

    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
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
    pub fn set_window_resize_callback(&mut self, callback: Option<fn(u32, u32)>) {
        unimplemented!()
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        unimplemented!()
    }

    #[inline]
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        unimplemented!()
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    #[inline]
    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        unimplemented!()
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }
}

impl GlContext for Window {
    #[inline]
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.context.make_current()
    }

    #[inline]
    fn is_current(&self) -> bool {
        self.context.is_current()
    }

    #[inline]
    fn get_proc_address(&self, addr: &str) -> *const () {
        self.context.get_proc_address(addr)
    }

    #[inline]
    fn swap_buffers(&self) -> Result<(), ContextError> {
        self.context.swap_buffers()
    }

    #[inline]
    fn get_api(&self) -> ::Api {
        self.context.get_api()
    }

    #[inline]
    fn get_pixel_format(&self) -> PixelFormat {
        self.context.get_pixel_format().clone()
    }
}
