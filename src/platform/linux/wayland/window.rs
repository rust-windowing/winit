use std::sync::{Arc, Mutex, Weak};

use {CreationError, CursorState, MouseCursor, WindowAttributes};
use platform::MonitorId as PlatformMonitorId;
use window::MonitorId as RootMonitorId;

use sctk::window::{BasicFrame, Event as WEvent, Window as SWindow};
use sctk::reexports::client::{Display, Proxy};
use sctk::reexports::client::protocol::{wl_seat, wl_surface};
use sctk::reexports::client::protocol::wl_compositor::RequestsTrait as CompositorRequests;
use sctk::reexports::client::protocol::wl_surface::RequestsTrait as SurfaceRequests;

use super::{make_wid, EventsLoop, MonitorId, WindowId};

pub struct Window {
    surface: Proxy<wl_surface::WlSurface>,
    frame: Arc<Mutex<SWindow<BasicFrame>>>,
    monitors: Arc<Mutex<Vec<MonitorId>>>,
    size: Arc<Mutex<(u32, u32)>>,
    kill_switch: (Arc<Mutex<bool>>, Arc<Mutex<bool>>),
    display: Arc<Display>,
    need_frame_refresh: Arc<Mutex<bool>>,
}

impl Window {
    pub fn new(evlp: &EventsLoop, attributes: WindowAttributes) -> Result<Window, CreationError> {
        let (width, height) = attributes.dimensions.unwrap_or((800, 600));
        // Create the window
        let size = Arc::new(Mutex::new((width, height)));

        // monitor tracking
        let monitor_list = Arc::new(Mutex::new(Vec::new()));

        let surface = evlp.env.compositor.create_surface().unwrap().implement({
            let list = monitor_list.clone();
            let omgr = evlp.env.outputs.clone();
            move |event, _| match event {
                wl_surface::Event::Enter { output } => list.lock().unwrap().push(MonitorId {
                    proxy: output,
                    mgr: omgr.clone(),
                }),
                wl_surface::Event::Leave { output } => {
                    list.lock().unwrap().retain(|m| !m.proxy.equals(&output));
                }
            }
        });

        let window_store = evlp.store.clone();
        let my_surface = surface.clone();
        let mut frame = SWindow::<BasicFrame>::init(
            surface.clone(),
            (width, height),
            &evlp.env.compositor,
            &evlp.env.subcompositor,
            &evlp.env.shm,
            &evlp.env.shell,
            move |event, ()| match event {
                WEvent::Configure { new_size, .. } => {
                    let mut store = window_store.lock().unwrap();
                    for window in &mut store.windows {
                        if window.surface.equals(&my_surface) {
                            window.newsize = new_size.map(|(w, h)| (w as i32, h as i32));
                            window.need_refresh = true;
                            *(window.need_frame_refresh.lock().unwrap()) = true;
                            return;
                        }
                    }
                }
                WEvent::Refresh => {
                    let store = window_store.lock().unwrap();
                    for window in &store.windows {
                        if window.surface.equals(&my_surface) {
                            *(window.need_frame_refresh.lock().unwrap()) = true;
                            return;
                        }
                    }
                }
                WEvent::Close => {
                    let mut store = window_store.lock().unwrap();
                    for window in &mut store.windows {
                        if window.surface.equals(&my_surface) {
                            window.closed = true;
                            return;
                        }
                    }
                }
            },
        ).unwrap();

        for &(_, ref seat) in evlp.seats.lock().unwrap().iter() {
            frame.new_seat(seat);
        }

        // Check for fullscreen requirements
        if let Some(RootMonitorId {
            inner: PlatformMonitorId::Wayland(ref monitor_id),
        }) = attributes.fullscreen
        {
            frame.set_fullscreen(Some(&monitor_id.proxy));
        } else if attributes.maximized {
            frame.set_maximized();
        }

        frame.set_resizable(attributes.resizable);

        // set decorations
        frame.set_decorate(attributes.decorations);

        // min-max dimensions
        frame.set_min_size(attributes.min_dimensions);
        frame.set_max_size(attributes.max_dimensions);

        let kill_switch = Arc::new(Mutex::new(false));
        let need_frame_refresh = Arc::new(Mutex::new(true));
        let frame = Arc::new(Mutex::new(frame));

        evlp.store.lock().unwrap().windows.push(InternalWindow {
            closed: false,
            newsize: None,
            need_refresh: false,
            need_frame_refresh: need_frame_refresh.clone(),
            surface: surface.clone(),
            kill_switch: kill_switch.clone(),
            frame: Arc::downgrade(&frame),
        });
        evlp.evq.borrow_mut().sync_roundtrip().unwrap();

        Ok(Window {
            display: evlp.display.clone(),
            surface: surface,
            frame: frame,
            monitors: monitor_list,
            size: size,
            kill_switch: (kill_switch, evlp.cleanup_needed.clone()),
            need_frame_refresh: need_frame_refresh,
        })
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        make_wid(&self.surface)
    }

    pub fn set_title(&self, title: &str) {
        self.frame.lock().unwrap().set_title(title.into());
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
    pub fn get_inner_position(&self) -> Option<(i32, i32)> {
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
        // let (w, h) = super::wayland_window::add_borders(w as i32, h as i32);
        Some((w as u32, h as u32))
    }

    #[inline]
    // NOTE: This will only resize the borders, the contents must be updated by the user
    pub fn set_inner_size(&self, x: u32, y: u32) {
        self.frame.lock().unwrap().resize(x, y);
        *(self.size.lock().unwrap()) = (x, y);
    }

    #[inline]
    pub fn set_min_dimensions(&self, dimensions: Option<(u32, u32)>) {
        self.frame.lock().unwrap().set_min_size(dimensions);
    }

    #[inline]
    pub fn set_max_dimensions(&self, dimensions: Option<(u32, u32)>) {
        self.frame.lock().unwrap().set_max_size(dimensions);
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        self.frame.lock().unwrap().set_resizable(resizable);
    }

    #[inline]
    pub fn set_cursor(&self, _cursor: MouseCursor) {
        // TODO
    }

    #[inline]
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        use CursorState::{Grab, Hide, Normal};
        // TODO : not yet possible on wayland to grab cursor
        match state {
            Grab => Err("Cursor cannot be grabbed on wayland yet.".to_string()),
            Hide => Err("Cursor cannot be hidden on wayland yet.".to_string()),
            Normal => Ok(()),
        }
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        let mut factor: f32 = 1.0;
        let guard = self.monitors.lock().unwrap();
        for monitor_id in guard.iter() {
            let hidpif = monitor_id.get_hidpi_factor();
            factor = factor.max(hidpif);
        }
        factor
    }

    pub fn set_decorations(&self, decorate: bool) {
        self.frame.lock().unwrap().set_decorate(decorate);
        *(self.need_frame_refresh.lock().unwrap()) = true;
    }

    pub fn set_maximized(&self, maximized: bool) {
        if maximized {
            self.frame.lock().unwrap().set_maximized();
        } else {
            self.frame.lock().unwrap().unset_maximized();
        }
    }

    pub fn set_fullscreen(&self, monitor: Option<RootMonitorId>) {
        if let Some(RootMonitorId {
            inner: PlatformMonitorId::Wayland(ref monitor_id),
        }) = monitor
        {
            self.frame
                .lock()
                .unwrap()
                .set_fullscreen(Some(&monitor_id.proxy));
        } else {
            self.frame.lock().unwrap().unset_fullscreen();
        }
    }

    #[inline]
    pub fn set_cursor_position(&self, _x: i32, _y: i32) -> Result<(), ()> {
        // TODO: not yet possible on wayland
        Err(())
    }

    pub fn get_display(&self) -> &Display {
        &*self.display
    }

    pub fn get_surface(&self) -> &Proxy<wl_surface::WlSurface> {
        &self.surface
    }

    pub fn get_current_monitor(&self) -> MonitorId {
        // we don't know how much each monitor sees us so...
        // just return the most recent one ?
        let guard = self.monitors.lock().unwrap();
        guard.last().unwrap().clone()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        *(self.kill_switch.0.lock().unwrap()) = true;
        *(self.kill_switch.1.lock().unwrap()) = true;
    }
}

/*
 * Internal store for windows
 */

struct InternalWindow {
    surface: Proxy<wl_surface::WlSurface>,
    newsize: Option<(i32, i32)>,
    need_refresh: bool,
    need_frame_refresh: Arc<Mutex<bool>>,
    closed: bool,
    kill_switch: Arc<Mutex<bool>>,
    frame: Weak<Mutex<SWindow<BasicFrame>>>,
}

pub struct WindowStore {
    windows: Vec<InternalWindow>,
}

impl WindowStore {
    pub fn new() -> WindowStore {
        WindowStore {
            windows: Vec::new(),
        }
    }

    pub fn find_wid(&self, surface: &Proxy<wl_surface::WlSurface>) -> Option<WindowId> {
        for window in &self.windows {
            if surface.equals(&window.surface) {
                return Some(make_wid(surface));
            }
        }
        None
    }

    pub fn cleanup(&mut self) -> Vec<WindowId> {
        let mut pruned = Vec::new();
        self.windows.retain(|w| {
            if *w.kill_switch.lock().unwrap() {
                // window is dead, cleanup
                pruned.push(make_wid(&w.surface));
                w.surface.destroy();
                false
            } else {
                true
            }
        });
        pruned
    }

    pub fn new_seat(&self, seat: &Proxy<wl_seat::WlSeat>) {
        for window in &self.windows {
            if let Some(w) = window.frame.upgrade() {
                w.lock().unwrap().new_seat(seat);
            }
        }
    }

    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(Option<(i32, i32)>, bool, bool, bool, WindowId, Option<&mut SWindow<BasicFrame>>),
    {
        for window in &mut self.windows {
            let opt_arc = window.frame.upgrade();
            let mut opt_mutex_lock = opt_arc.as_ref().map(|m| m.lock().unwrap());
            f(
                window.newsize.take(),
                window.need_refresh,
                ::std::mem::replace(&mut *window.need_frame_refresh.lock().unwrap(), false),
                window.closed,
                make_wid(&window.surface),
                opt_mutex_lock.as_mut().map(|m| &mut **m),
            );
            window.need_refresh = false;
            // avoid re-spamming the event
            window.closed = false;
        }
    }
}
