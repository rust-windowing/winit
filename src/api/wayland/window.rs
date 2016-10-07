use std::sync::Arc;

use wayland_client::protocol::{wl_display,wl_surface};

use {CreationError, MouseCursor, CursorState, Event, WindowAttributes};

use super::WaylandContext;

#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        unimplemented!()
    }
}

pub struct Window {
    resize_callback: Option<fn(u32,u32)>
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
    pub fn new(ctxt: Arc<WaylandContext>, attributes: &WindowAttributes)  -> Result<Window, CreationError>
    {
        unimplemented!()
    }

    pub fn set_title(&self, title: &str) {
        // TODO
    }

    #[inline]
    pub fn show(&self) {
        // TODO
    }

    #[inline]
    pub fn hide(&self) {
        // TODO
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        // Not possible with wayland
        None
    }

    #[inline]
    pub fn set_position(&self, _x: i32, _y: i32) {
        // Not possible with wayland
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        // TODO
        None
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        // TODO
        None
    }

    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        // TODO
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
        self.resize_callback = callback;
    }

    #[inline]
    pub fn set_cursor(&self, _cursor: MouseCursor) {
        // TODO
    }

    #[inline]
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        use CursorState::{Grab, Normal, Hide};
        // TODO : not yet possible on wayland to grab cursor
        match state {
            Grab => Err("Cursor cannot be grabbed on wayland yet.".to_string()),
            Hide => Err("Cursor cannot be hidden on wayland yet.".to_string()),
            Normal => Ok(())
        }
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        // TODO
        1.0
    }

    #[inline]
    pub fn set_cursor_position(&self, _x: i32, _y: i32) -> Result<(), ()> {
        // TODO: not yet possible on wayland
        Err(())
    }
    
    pub fn get_display(&self) -> &wl_display::WlDisplay {
        unimplemented!()
    }
    
    pub fn get_surface(&self) -> &wl_surface::WlSurface {
        unimplemented!()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // TODO
    }
}
