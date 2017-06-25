use std::fs::File;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;

use wayland_client::{EventQueue, EventQueueHandle, Proxy};
use wayland_client::protocol::{wl_display, wl_output, wl_surface, wl_shell_surface,wl_buffer};

use {CreationError, MouseCursor, CursorState, WindowAttributes};
use platform::MonitorId as PlatformMonitorId;

use super::{WaylandContext, EventsLoop};
use super::context::WaylandEnv;
use super::wayland_window;
use super::wayland_window::DecoratedSurface;

pub struct Window {
    // the global wayland context
    ctxt: Arc<WaylandContext>,
    // the EventQueue of our EventsLoop
    evq: Arc<Mutex<EventQueue>>,
    // signal to advertize the EventsLoop when we are destroyed
    cleanup_signal: Arc<AtomicBool>,
    // our wayland surface
    surface: Arc<wl_surface::WlSurface>,
    // our current inner dimensions
    size: Mutex<(u32, u32)>,
    // the id of our DecoratedHandler in the EventQueue
    decorated_id: usize,
    // the id of our SurfaceHandler in the EventQueue
    surface_handler_id: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(usize);

#[inline]
pub fn make_wid(s: &wl_surface::WlSurface) -> WindowId {
    WindowId(s.ptr() as usize)
}

impl Window {
    pub fn new(
        evlp: &EventsLoop,
        ctxt: Arc<WaylandContext>,
        attributes: &WindowAttributes,
    ) -> Result<Self, CreationError>
    {
        let (width, height) = attributes.dimensions.unwrap_or((800,600));

        let (surface, decorated, buffer, tmpfile) = ctxt.create_window::<DecoratedHandler>(width, height);

        // init DecoratedSurface
        let (evq, cleanup_signal) = evlp.get_window_init();

        // Create and register the surface handler, used to track the monitors that overlap the
        // window.
        let surface_handler_id = {
            let mut evq_guard = evq.lock().unwrap();
            let surface_handler_id = evq_guard.add_handler(SurfaceHandler {
                ctxt: ctxt.clone(),
                intersecting_monitors: vec![],
            });
            // Register the window's surface to it.
            evq_guard.register::<_, SurfaceHandler>(&surface, surface_handler_id);
            surface_handler_id
        };

        let decorated_id = {
            let mut evq_guard = evq.lock().unwrap();
            // create a handler to clean up initial buffer
            let initial_buffer_handler_id = evq_guard.add_handler(InitialBufferHandler::new());
            // register the buffer to it
            evq_guard.register::<_, InitialBufferHandler>(&buffer, initial_buffer_handler_id);
            // store the DecoratedSurface handler
            let decorated_id = evq_guard.add_handler_with_init(decorated);
            {
                let mut state = evq_guard.state();
                {
                    // store the buffer and tempfile in the handler, to be cleanded up at the right
                    // time
                    let initial_buffer_h = state.get_mut_handler::<InitialBufferHandler>(initial_buffer_handler_id);
                    initial_buffer_h.initial_buffer = Some((buffer, tmpfile));
                }
                // initialize the DecoratedHandler
                let decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(decorated_id);
                *(decorated.handler()) = Some(DecoratedHandler::new());

                // set fullscreen if necessary
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
            decorated_id
        };

        let window = Window {
            ctxt: ctxt,
            evq: evq,
            cleanup_signal: cleanup_signal,
            surface: surface,
            size: Mutex::new((width, height)),
            decorated_id: decorated_id,
            surface_handler_id: surface_handler_id,
        };

        // register the window with the EventsLoop
        evlp.register_window(window.decorated_id, window.surface.clone());

        Ok(window)
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        make_wid(&self.surface)
    }

    pub fn set_title(&self, title: &str) {
        let mut guard = self.evq.lock().unwrap();
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
        let mut guard = self.evq.lock().unwrap();
        let mut state = guard.state();
        let mut decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(self.decorated_id);
        decorated.resize(x as i32, y as i32);
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

    pub fn monitor_id(&self) -> Option<super::MonitorId> {
        let mut guard = self.evq.lock().unwrap();
        let state = guard.state();
        let surface_handler = state.get_handler::<SurfaceHandler>(self.surface_handler_id);
        surface_handler
            .intersecting_monitors
            .get(0)
            .map(|&id| super::MonitorId { id: id, ctxt: self.ctxt.clone() })
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.surface.destroy();
        self.cleanup_signal.store(true, ::std::sync::atomic::Ordering::Relaxed);
    }
}

pub struct DecoratedHandler {
    newsize: Option<(u32, u32)>
}

impl DecoratedHandler {
    fn new() -> DecoratedHandler { DecoratedHandler { newsize: None }}

    pub fn take_newsize(&mut self) -> Option<(u32, u32)> {
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

// a handler to release the ressources acquired to draw the initial white screen as soon as
// the compositor does not use them any more

pub struct InitialBufferHandler {
    initial_buffer: Option<(wl_buffer::WlBuffer, File)>
}

impl InitialBufferHandler {
    fn new() -> InitialBufferHandler {
        InitialBufferHandler {
            initial_buffer: None,
        }
    }
}


impl wl_buffer::Handler for InitialBufferHandler {
    fn release(&mut self, _: &mut EventQueueHandle, buffer: &wl_buffer::WlBuffer) {
        // release the ressources we've acquired for initial white window
        buffer.destroy();
        self.initial_buffer = None;
    }
}

declare_handler!(InitialBufferHandler, wl_buffer::Handler, wl_buffer::WlBuffer);

// A handler to track a window's surface intersections with the available outputs (monitors).

struct SurfaceHandler {
    // The id of each monitor that currently intersects the window.
    // The order represents the chronological order in which intersections appeared.
    intersecting_monitors: Vec<u32>,
    // The global wayland context, used to find the output ID.
    ctxt: Arc<WaylandContext>,
}

impl SurfaceHandler {
    fn monitor_id(&self, output: &wl_output::WlOutput) -> Option<u32> {
        let mut guard = self.ctxt.evq.lock().unwrap();
        let state = guard.state();
        let env = state.get_handler::<WaylandEnv>(self.ctxt.env_id);
        env.monitors.iter()
            .find(|m| m.output.equals(output))
            .map(|m| m.id)
    }
}

impl wl_surface::Handler for SurfaceHandler {
    fn enter(
        &mut self,
        _evqh: &mut EventQueueHandle,
        _proxy: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        if let Some(id) = self.monitor_id(output) {
            self.intersecting_monitors.push(id);
        }
    }

    fn leave(
        &mut self,
        _evqh: &mut EventQueueHandle,
        _proxy: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        if let Some(monitor_id) = self.monitor_id(output) {
            self.intersecting_monitors.retain(|&id| id != monitor_id);
        }
    }
}

declare_handler!(SurfaceHandler, wl_surface::Handler, wl_surface::WlSurface);
