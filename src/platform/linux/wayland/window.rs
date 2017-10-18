use std::sync::{Arc, Mutex};

use wayland_client::protocol::{wl_display,wl_surface};
use wayland_client::{Proxy, StateToken};

use {CreationError, MouseCursor, CursorState, WindowAttributes};
use platform::MonitorId as PlatformMonitorId;
use window::MonitorId as RootMonitorId;

use super::{EventsLoop, WindowId, make_wid, MonitorId};
use super::wayland_window::{DecoratedSurface, DecoratedSurfaceImplementation};
use super::event_loop::StateContext;

pub struct Window {
    display: Arc<wl_display::WlDisplay>,
    surface: wl_surface::WlSurface,
    decorated: Mutex<DecoratedSurface>,
    monitors: Arc<Mutex<MonitorList>>,
    ready: Arc<Mutex<bool>>,
    size: Arc<Mutex<(u32, u32)>>
}

impl Window {
    pub fn new(evlp: &EventsLoop, attributes: &WindowAttributes) -> Result<Window, CreationError>
    {
        let (width, height) = attributes.dimensions.unwrap_or((800,600));

        // Create the decorated surface
        let ready = Arc::new(Mutex::new(false));
        let size = Arc::new(Mutex::new((width, height)));
        let store_token = evlp.store.clone();
        let (surface, mut decorated, xdg) = evlp.create_window(
            width, height, attributes.decorations, decorated_impl(),
            |surface| DecoratedIData {
                ready: ready.clone(),
                surface: surface.clone().unwrap(),
                store_token: store_token.clone()
            }
        );
        // If we are using xdg, we are not ready yet
        { *ready.lock().unwrap() = !xdg; }
        // Check for fullscreen requirements
        if let Some(RootMonitorId { inner: PlatformMonitorId::Wayland(ref monitor_id) }) = attributes.fullscreen {
            let info = monitor_id.info.lock().unwrap();
            decorated.set_fullscreen(Some(&info.output));
        } else if !attributes.decorations {
            decorated.set_decorate(false);
        }
        // setup the monitor tracking
        let monitor_list = Arc::new(Mutex::new(MonitorList::default()));
        {
            let mut evq = evlp.evq.borrow_mut();
            let idata = (evlp.ctxt_token.clone(), monitor_list.clone());
            evq.register(&surface, surface_impl(), idata);
        }
        // a surface commit with no buffer so that the compositor don't
        // forget to configure us
        surface.commit();

        {
            let mut evq = evlp.evq.borrow_mut();
            evq.state().get_mut(&store_token).windows.push(InternalWindow {
                closed: false,
                newsize: None,
                surface: surface.clone().unwrap()
            });
            evq.sync_roundtrip().unwrap();
        }

        Ok(Window {
            display: evlp.display.clone(),
            surface: surface,
            decorated: Mutex::new(decorated),
            monitors: monitor_list,
            ready: ready,
            size: size
        })
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        make_wid(&self.surface)
    }

    pub fn set_title(&self, title: &str) {
        self.decorated.lock().unwrap().set_title(title.into());
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
        self.decorated.lock().unwrap().resize(x as i32, y as i32);
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
        let mut factor = 1.0;
        let guard = self.monitors.lock().unwrap();
        for monitor_id in &guard.monitors {
            let info = monitor_id.info.lock().unwrap();
            if info.scale > factor { factor = info.scale; }
        }
        factor
    }

    #[inline]
    pub fn set_cursor_position(&self, _x: i32, _y: i32) -> Result<(), ()> {
        // TODO: not yet possible on wayland
        Err(())
    }
    
    pub fn get_display(&self) -> &wl_display::WlDisplay {
        &*self.display
    }
    
    pub fn get_surface(&self) -> &wl_surface::WlSurface {
        &self.surface
    }

    pub fn get_current_monitor(&self) -> MonitorId {
        // we don't know how much each monitor sees us so...
        // just return the most recent one ?
        let guard = self.monitors.lock().unwrap();
        guard.monitors.last().unwrap().clone()
    }

    pub fn is_ready(&self) -> bool {
        *self.ready.lock().unwrap()
    }
}

/*
 * Internal store for windows
 */

struct InternalWindow {
    surface: wl_surface::WlSurface,
    newsize: Option<(i32, i32)>,
    closed: bool
}

pub struct WindowStore {
    windows: Vec<InternalWindow>
}

impl WindowStore {
    pub fn new() -> WindowStore {
        WindowStore { windows: Vec::new() }
    }

    pub fn find_wid(&self, surface: &wl_surface::WlSurface) -> Option<WindowId> {
        for window in &self.windows {
            if surface.equals(&window.surface) {
                return Some(make_wid(surface));
            }
        }
        None
    }
}

/*
 * Protocol implementation
 */

struct DecoratedIData {
    ready: Arc<Mutex<bool>>,
    store_token: StateToken<WindowStore>,
    surface: wl_surface::WlSurface
}

fn decorated_impl() -> DecoratedSurfaceImplementation<DecoratedIData> {
    DecoratedSurfaceImplementation {
        configure: |evqh, idata, _, newsize| {
            *idata.ready.lock().unwrap() = true;
            if let Some(newsize) = newsize {
                let store = evqh.state().get_mut(&idata.store_token);
                for window in &mut store.windows {
                    if window.surface.equals(&idata.surface) {
                        window.newsize = Some(newsize);
                        return;
                    }
                }
            }
        },
        close: |evqh, idata| {
            let store = evqh.state().get_mut(&idata.store_token);
            for window in &mut store.windows {
                if window.surface.equals(&idata.surface) {
                    window.closed = true;
                    return;
                }
            }
        }
    }
}

#[derive(Default)]
struct MonitorList {
    monitors: Vec<MonitorId>
}

fn surface_impl() -> wl_surface::Implementation<(StateToken<StateContext>, Arc<Mutex<MonitorList>>)> {
    wl_surface::Implementation {
        enter: |evqh, &mut (ref token, ref list), _, output| {
            let mut guard = list.lock().unwrap();
            let ctxt = evqh.state().get(token);
            let monitor = ctxt.monitor_id_for(output);
            guard.monitors.push(monitor);
        },
        leave: |evqh, &mut (ref token, ref list), _, output| {
            let mut guard = list.lock().unwrap();
            let ctxt = evqh.state().get(token);
            let monitor = ctxt.monitor_id_for(output);
            guard.monitors.retain(|m| !Arc::ptr_eq(&m.info, &monitor.info));
        }
    }
}