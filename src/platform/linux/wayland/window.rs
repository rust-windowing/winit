use std::sync::{Arc, Mutex};
use std::sync::atomic::{Ordering, AtomicBool};
use std::cmp;

use wayland_client::{EventQueueHandle, Proxy};
use wayland_client::protocol::{wl_display,wl_surface};

use {CreationError, MouseCursor, CursorState, WindowAttributes};
use platform::MonitorId as PlatformMonitorId;
use window::MonitorId as RootMonitorId;
use platform::wayland::MonitorId as WaylandMonitorId;
use platform::wayland::context::get_available_monitors;

use super::{WaylandContext, EventsLoop};
use super::wayland_window;
use super::wayland_window::DecoratedSurface;

pub struct Window {
    // the global wayland context
    ctxt: Arc<WaylandContext>,
    // signal to advertize the EventsLoop when we are destroyed
    cleanup_signal: Arc<AtomicBool>,
    // our wayland surface
    surface: Arc<wl_surface::WlSurface>,
    // our current inner dimensions
    size: Mutex<(u32, u32)>,
    // the id of our DecoratedHandler in the EventQueue
    decorated_id: usize
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(usize);

#[inline]
pub fn make_wid(s: &wl_surface::WlSurface) -> WindowId {
    WindowId(s.ptr() as usize)
}

impl Window {
    pub fn new(evlp: &EventsLoop, attributes: &WindowAttributes) -> Result<Window, CreationError>
    {
        let ctxt = evlp.context().clone();
        let (width, height) = attributes.dimensions.unwrap_or((800,600));

        let (surface, decorated, xdg) = ctxt.create_window::<DecoratedHandler>(width, height, attributes.decorations);

        // init DecoratedSurface
        let cleanup_signal = evlp.get_window_init();

        let mut fullscreen_monitor = None;
        if let Some(RootMonitorId { inner: PlatformMonitorId::Wayland(ref monitor_id) }) = attributes.fullscreen {
            ctxt.with_output(monitor_id.clone(), |output| {
                fullscreen_monitor = output.clone();
            });
        }

        let decorated_id = {
            let mut evq_guard = ctxt.evq.lock().unwrap();
            // store the DecoratedSurface handler
            let decorated_id = evq_guard.add_handler_with_init(decorated);
            {
                let mut state = evq_guard.state();
                // initialize the DecoratedHandler
                let decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(decorated_id);
                *(decorated.handler()) = Some(DecoratedHandler::new(!xdg));

                // set fullscreen if necessary
                if let Some(output) = fullscreen_monitor {
                    decorated.set_fullscreen(Some(&output));
                }
                // Finally, set the decorations size
                decorated.resize(width as i32, height as i32);
            }

            evq_guard.sync_roundtrip().unwrap();

            decorated_id
        };
        // send our configuration to the compositor
        // if we're in xdg mode, no buffer is attached yet, so this
        // is fine (and more or less required actually)
        surface.commit();
        let me = Window {
            ctxt: ctxt,
            cleanup_signal: cleanup_signal,
            surface: surface,
            size: Mutex::new((width, height)),
            decorated_id: decorated_id
        };

        // register ourselves to the EventsLoop
        evlp.register_window(me.decorated_id, me.surface.clone());

        Ok(me)
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        make_wid(&self.surface)
    }

    pub fn set_title(&self, title: &str) {
        let mut guard = self.ctxt.evq.lock().unwrap();
        let mut state = guard.state();
        let decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(self.decorated_id);
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
        let mut guard = self.ctxt.evq.lock().unwrap();
        let mut state = guard.state();
        let mut decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(self.decorated_id);
        decorated.resize(x as i32, y as i32);
        *(self.size.lock().unwrap()) = (x, y);
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

    pub fn get_current_monitor(&self) -> WaylandMonitorId {
        let monitors = get_available_monitors(&self.ctxt);
        let default = monitors[0].clone();

        let (wx,wy) = match self.get_position() {
            Some(val) => (cmp::max(0,val.0) as u32, cmp::max(0,val.1) as u32),
            None=> return default,
        };
        let (ww,wh) = match self.get_outer_size() {
            Some(val) => val,
            None=> return default,
        };
        // Opposite corner coordinates
        let (wxo, wyo) = (wx+ww-1, wy+wh-1);

        // Find the monitor with the biggest overlap with the window
        let mut overlap = 0;
        let mut find = default;
        for monitor in monitors {
            let (mx, my) = monitor.get_position();
            let (mw, mh) = monitor.get_dimensions();
            let (mxo, myo) = (mx+mw-1, my+mh-1);
            let (ox, oy) = (cmp::max(wx, mx), cmp::max(wy, my));
            let (oxo, oyo) = (cmp::min(wxo, mxo), cmp::min(wyo, myo));
            let osize = if ox <= oxo || oy <= oyo { 0 } else { (oxo-ox)*(oyo-oy) };

            if osize > overlap {
                overlap = osize;
                find = monitor;
            }
        }

        find
    }

    pub fn is_ready(&self) -> bool {
        let mut guard = self.ctxt.evq.lock().unwrap();
        let mut state = guard.state();
        let mut decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(self.decorated_id);
        decorated.handler().as_ref().unwrap().configured
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.surface.destroy();
        self.cleanup_signal.store(true, Ordering::Relaxed);
    }
}

pub struct DecoratedHandler {
    newsize: Option<(u32, u32)>,
    refresh: bool,
    closed: bool,
    configured: bool
}

impl DecoratedHandler {
    fn new(configured: bool) -> DecoratedHandler {
        DecoratedHandler {
            newsize: None,
            refresh: false,
            closed: false,
            configured: configured
        }
    }

    pub fn take_newsize(&mut self) -> Option<(u32, u32)> {
        self.newsize.take()
    }

    pub fn take_refresh(&mut self) -> bool {
        let refresh = self.refresh;
        self.refresh = false;
        refresh
    }

    pub fn is_closed(&self) -> bool { self.closed }
}

impl wayland_window::Handler for DecoratedHandler {
    fn configure(&mut self,
                 _: &mut EventQueueHandle,
                 _cfg: wayland_window::Configure,
                 newsize: Option<(i32, i32)>)
    {
        self.newsize = newsize.map(|(w, h)| (w as u32, h as u32));
        self.refresh = true;
        self.configured = true;
    }

    fn close(&mut self, _: &mut EventQueueHandle) {
        self.closed = true;
    }
}


