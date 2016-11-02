use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use wayland_client::{EventQueue, EventQueueHandle, Init};
use wayland_client::protocol::{wl_display,wl_surface,wl_shell_surface};

use {CreationError, MouseCursor, CursorState, Event, WindowAttributes};
use platform::MonitorId as PlatformMonitorId;

use super::WaylandContext;
use super::wayland_window;
use super::wayland_window::DecoratedSurface;

#[derive(Clone)]
pub struct WindowProxy {
    ctxt: Arc<WaylandContext>,
    eviter: Arc<Mutex<VecDeque<Event>>>,
}

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        // Send a sync event, so that any waiting "dispatch" will return
        self.ctxt.display.sync();
        self.eviter.lock().unwrap().push_back(Event::Awakened);
    }
}

pub struct Window {
    ctxt: Arc<WaylandContext>,
    evq: Mutex<EventQueue>,
    eviter: Arc<Mutex<VecDeque<Event>>>,
    surface: Arc<wl_surface::WlSurface>,
    size: Mutex<(u32, u32)>,
    handler_id: usize,
    decorated_id: usize
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        self.window.next_event(false)
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        self.window.next_event(true)
    }
}

impl Window {
    pub fn new(ctxt: Arc<WaylandContext>, attributes: &WindowAttributes)  -> Result<Window, CreationError>
    {
        let (width, height) = attributes.dimensions.unwrap_or((800,600));

        let mut evq = ctxt.display.create_event_queue();

        let (surface, eviter, decorated) = ctxt.create_window::<DecoratedHandler>();

        // init DecoratedSurface
        let decorated_id = evq.add_handler_with_init(decorated);
        {
            let mut state = evq.state();
            let decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(decorated_id);
            *(decorated.handler()) = Some(DecoratedHandler::new());

            if let Some(PlatformMonitorId::Wayland(ref monitor_id)) = attributes.monitor {
                ctxt.with_output(monitor_id.clone(), |output| {
                    decorated.set_fullscreen(
                        wl_shell_surface::FullscreenMethod::Default,
                        0,
                        Some(output)
                    )
                });
            } else if attributes.decorations {
                decorated.set_decorate(true);
            }
            // Finally, set the decorations size
            decorated.resize(width as i32, height as i32);
        }

        // init general handler
        let handler = WindowHandler::new();
        let handler_id = evq.add_handler_with_init(handler);

        Ok(Window {
            ctxt: ctxt,
            evq: Mutex::new(evq),
            eviter: eviter,
            surface: surface,
            size: Mutex::new((width, height)),
            handler_id: handler_id,
            decorated_id: decorated_id
        })
    }

    fn process_resize(&self) {
        use std::cmp::max;
        let mut evq_guard = self.evq.lock().unwrap();
        let mut state = evq_guard.state();
        let newsize = {
            let decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(self.decorated_id);
            let newsize = decorated.handler().as_mut().and_then(|h| h.take_newsize());
            if let Some((w, h)) = newsize {
                decorated.resize(w as i32, h as i32);
                *self.size.lock().unwrap() = (w, h);
            }
            newsize
        };
        // callback_resize if any
        if let Some((w, h)) = newsize {
            let mut handler = state.get_mut_handler::<WindowHandler>(self.handler_id);
            if let Some(ref callback) = handler.resize_callback {
                callback(w, h);
            }
            self.eviter.lock().unwrap().push_back(Event::Resized(w,h));
        }
    }

    fn next_event(&self, block: bool) -> Option<Event> {
        let mut evt = {
            let mut guard = self.eviter.lock().unwrap();
            guard.pop_front()
        };
        if evt.is_some() { return evt }

        // There is no event in the queue, we need to fetch more

        // flush the display
        self.ctxt.flush();

        // read some events if some are waiting & queue is empty
        if let Some(guard) = self.evq.lock().unwrap().prepare_read() {
            guard.read_events();
        }

        // try a pending dispatch
        {
            self.ctxt.dispatch_pending();
            self.evq.lock().unwrap().dispatch_pending();
            // some events were dispatched, need to process a potential resising
            self.process_resize();
        }

        let mut evt = {
            let mut guard = self.eviter.lock().unwrap();
            guard.pop_front()
        };

        while block && evt.is_none() {
            // no event waiting, need to repopulate!
            {
                self.ctxt.flush();
                self.ctxt.dispatch();
                self.evq.lock().unwrap().dispatch_pending();
                // some events were dispatched, need to process a potential resising
                self.process_resize();
            }
            // try again
            let mut guard = self.eviter.lock().unwrap();
            evt = guard.pop_front();
        }
        evt
    }

    pub fn set_title(&self, title: &str) {
        let mut guard = self.evq.lock().unwrap();
        let mut state = guard.state();
        let mut decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(self.decorated_id);
        decorated.set_title(title.into())
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
        Some(self.size.lock().unwrap().clone())
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        let (w, h) = self.size.lock().unwrap().clone();
        let (w, h) = super::wayland_window::add_borders(w as i32, h as i32);
        Some((w as u32, h as u32))
    }

    #[inline]
    // NOTE: This will only resize the borders, the contents must be updated by the user
    pub fn set_inner_size(&self, x: u32, y: u32) {
        let mut guard = self.evq.lock().unwrap();
        let mut state = guard.state();
        let mut decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(self.decorated_id);
        decorated.resize(x as i32, y as i32);
    }

    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy {
            ctxt: self.ctxt.clone(),
            eviter: self.eviter.clone()
        }
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
        let mut guard = self.evq.lock().unwrap();
        let mut state = guard.state();
        let mut handler = state.get_mut_handler::<WindowHandler>(self.handler_id);
        handler.resize_callback = callback;
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
        &self.ctxt.display
    }
    
    pub fn get_surface(&self) -> &wl_surface::WlSurface {
        &self.surface
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.surface.destroy();
        self.ctxt.prune_dead_windows();
    }
}

struct DecoratedHandler {
    newsize: Option<(u32, u32)>
}

impl DecoratedHandler {
    fn new() -> DecoratedHandler { DecoratedHandler { newsize: None }}
    fn take_newsize(&mut self) -> Option<(u32, u32)> {
        self.newsize.take()
    }
}

impl wayland_window::Handler for DecoratedHandler {
    fn configure(&mut self,
                 _: &mut EventQueueHandle,
                 _: wl_shell_surface::Resize,
                 width: i32, height: i32)
    {
        use std::cmp::max;
        self.newsize = Some((max(width,1) as u32, max(height,1) as u32));
    }
}

struct WindowHandler {
    my_id: usize,
    resize_callback: Option<fn(u32,u32)>,
}

impl WindowHandler {
    fn new() -> WindowHandler {
        WindowHandler {
            my_id: 0,
            resize_callback: None
        }
    }
}

impl Init for WindowHandler {
    fn init(&mut self, evqh: &mut EventQueueHandle, index: usize) {
        self.my_id = index;
    }
}
