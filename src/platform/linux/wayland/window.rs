use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};

use {CreationError, MouseCursor, WindowAttributes};
use dpi::{LogicalPosition, LogicalSize};
use platform::MonitorId as PlatformMonitorId;
use window::MonitorId as RootMonitorId;

use sctk::window::{BasicFrame, Event as WEvent, Window as SWindow};
use sctk::reexports::client::{Display, Proxy};
use sctk::reexports::client::protocol::{wl_seat, wl_surface, wl_output};
use sctk::reexports::client::protocol::wl_compositor::RequestsTrait as CompositorRequests;
use sctk::reexports::client::protocol::wl_surface::RequestsTrait as SurfaceRequests;
use sctk::output::OutputMgr;

use super::{make_wid, EventsLoop, MonitorId, WindowId};
use platform::platform::wayland::event_loop::{get_available_monitors, get_primary_monitor};

pub struct Window {
    surface: Proxy<wl_surface::WlSurface>,
    frame: Arc<Mutex<SWindow<BasicFrame>>>,
    monitors: Arc<Mutex<MonitorList>>, // Monitors this window is currently on
    outputs: OutputMgr, // Access to info for all monitors
    size: Arc<Mutex<(u32, u32)>>,
    kill_switch: (Arc<Mutex<bool>>, Arc<Mutex<bool>>),
    display: Arc<Display>,
    need_frame_refresh: Arc<Mutex<bool>>,
}

impl Window {
    pub fn new(evlp: &EventsLoop, attributes: WindowAttributes) -> Result<Window, CreationError> {
        let (width, height) = attributes.dimensions.map(Into::into).unwrap_or((800, 600));
        // Create the window
        let size = Arc::new(Mutex::new((width, height)));

        // monitor tracking
        let monitor_list = Arc::new(Mutex::new(MonitorList::new()));

        let surface = evlp.env.compositor.create_surface().unwrap().implement({
            let list = monitor_list.clone();
            let omgr = evlp.env.outputs.clone();
            let window_store = evlp.store.clone();
            move |event, surface: Proxy<wl_surface::WlSurface>| match event {
                wl_surface::Event::Enter { output } => {
                    let dpi_change = list.lock().unwrap().add_output(MonitorId {
                        proxy: output,
                        mgr: omgr.clone(),
                    });
                    if let Some(dpi) = dpi_change {
                        if surface.version() >= 3 {
                            // without version 3 we can't be dpi aware
                            window_store.lock().unwrap().dpi_change(&surface, dpi);
                            surface.set_buffer_scale(dpi);
                        }
                    }
                },
                wl_surface::Event::Leave { output } => {
                    let dpi_change = list.lock().unwrap().del_output(&output);
                    if let Some(dpi) = dpi_change {
                        if surface.version() >= 3 {
                            // without version 3 we can't be dpi aware
                            window_store.lock().unwrap().dpi_change(&surface, dpi);
                            surface.set_buffer_scale(dpi);
                        }
                    }
                }
            }
        });

        let window_store = evlp.store.clone();
        let my_surface = surface.clone();
        let mut frame = SWindow::<BasicFrame>::init_from_env(
            &evlp.env,
            surface.clone(),
            (width, height),
            move |event, ()| match event {
                WEvent::Configure { new_size, .. } => {
                    let mut store = window_store.lock().unwrap();
                    for window in &mut store.windows {
                        if window.surface.equals(&my_surface) {
                            window.newsize = new_size;
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
        frame.set_min_size(attributes.min_dimensions.map(Into::into));
        frame.set_max_size(attributes.max_dimensions.map(Into::into));

        let kill_switch = Arc::new(Mutex::new(false));
        let need_frame_refresh = Arc::new(Mutex::new(true));
        let frame = Arc::new(Mutex::new(frame));

        evlp.store.lock().unwrap().windows.push(InternalWindow {
            closed: false,
            newsize: None,
            size: size.clone(),
            need_refresh: false,
            need_frame_refresh: need_frame_refresh.clone(),
            surface: surface.clone(),
            kill_switch: kill_switch.clone(),
            frame: Arc::downgrade(&frame),
            current_dpi: 1,
            new_dpi: None,
        });
        evlp.evq.borrow_mut().sync_roundtrip().unwrap();

        Ok(Window {
            display: evlp.display.clone(),
            surface: surface,
            frame: frame,
            monitors: monitor_list,
            outputs: evlp.env.outputs.clone(),
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
    pub fn get_position(&self) -> Option<LogicalPosition> {
        // Not possible with wayland
        None
    }

    #[inline]
    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        // Not possible with wayland
        None
    }

    #[inline]
    pub fn set_position(&self, _pos: LogicalPosition) {
        // Not possible with wayland
    }

    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        Some(self.size.lock().unwrap().clone().into())
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        let (w, h) = self.size.lock().unwrap().clone();
        // let (w, h) = super::wayland_window::add_borders(w as i32, h as i32);
        Some((w, h).into())
    }

    #[inline]
    // NOTE: This will only resize the borders, the contents must be updated by the user
    pub fn set_inner_size(&self, size: LogicalSize) {
        let (w, h) = size.into();
        self.frame.lock().unwrap().resize(w, h);
        *(self.size.lock().unwrap()) = (w, h);
    }

    #[inline]
    pub fn set_min_dimensions(&self, dimensions: Option<LogicalSize>) {
        self.frame.lock().unwrap().set_min_size(dimensions.map(Into::into));
    }

    #[inline]
    pub fn set_max_dimensions(&self, dimensions: Option<LogicalSize>) {
        self.frame.lock().unwrap().set_max_size(dimensions.map(Into::into));
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        self.frame.lock().unwrap().set_resizable(resizable);
    }

    #[inline]
    pub fn hidpi_factor(&self) -> i32 {
        self.monitors.lock().unwrap().compute_hidpi_factor()
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
    pub fn set_cursor(&self, _cursor: MouseCursor) {
        // TODO
    }

    #[inline]
    pub fn hide_cursor(&self, _hide: bool) {
        // TODO: This isn't possible on Wayland yet
    }

    #[inline]
    pub fn grab_cursor(&self, _grab: bool) -> Result<(), String> {
        Err("Cursor grabbing is not yet possible on Wayland.".to_owned())
    }

    #[inline]
    pub fn set_cursor_position(&self, _pos: LogicalPosition) -> Result<(), String> {
        Err("Setting the cursor position is not yet possible on Wayland.".to_owned())
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
        guard.monitors.last().unwrap().clone()
    }

    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        get_available_monitors(&self.outputs)
    }

    pub fn get_primary_monitor(&self) -> MonitorId {
        get_primary_monitor(&self.outputs)
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
    newsize: Option<(u32, u32)>,
    size: Arc<Mutex<(u32, u32)>>,
    need_refresh: bool,
    need_frame_refresh: Arc<Mutex<bool>>,
    closed: bool,
    kill_switch: Arc<Mutex<bool>>,
    frame: Weak<Mutex<SWindow<BasicFrame>>>,
    current_dpi: i32,
    new_dpi: Option<i32>
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

    fn dpi_change(&mut self, surface: &Proxy<wl_surface::WlSurface>, new: i32) {
        for window in &mut self.windows {
            if surface.equals(&window.surface) {
                window.new_dpi = Some(new);
            }
        }
    }

    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(Option<(u32, u32)>, &mut (u32, u32), Option<i32>, bool, bool, bool, WindowId, Option<&mut SWindow<BasicFrame>>),
    {
        for window in &mut self.windows {
            let opt_arc = window.frame.upgrade();
            let mut opt_mutex_lock = opt_arc.as_ref().map(|m| m.lock().unwrap());
            f(
                window.newsize.take(),
                &mut *(window.size.lock().unwrap()),
                window.new_dpi,
                window.need_refresh,
                ::std::mem::replace(&mut *window.need_frame_refresh.lock().unwrap(), false),
                window.closed,
                make_wid(&window.surface),
                opt_mutex_lock.as_mut().map(|m| &mut **m),
            );
            if let Some(dpi) = window.new_dpi.take() {
                window.current_dpi = dpi;
            }
            window.need_refresh = false;
            // avoid re-spamming the event
            window.closed = false;
        }
    }
}

/*
 * Monitor list with some covenience method to compute DPI
 */

struct MonitorList {
    monitors: Vec<MonitorId>
}

impl MonitorList {
    fn new() -> MonitorList {
        MonitorList {
            monitors: Vec::new()
        }
    }

    fn compute_hidpi_factor(&self) -> i32 {
        let mut factor = 1;
        for monitor_id in &self.monitors {
            let monitor_dpi = monitor_id.get_hidpi_factor();
            if monitor_dpi > factor { factor = monitor_dpi; }
        }
        factor
    }

    fn add_output(&mut self, monitor: MonitorId) -> Option<i32> {
        let old_dpi = self.compute_hidpi_factor();
        let monitor_dpi = monitor.get_hidpi_factor();
        self.monitors.push(monitor);
        if monitor_dpi > old_dpi {
            Some(monitor_dpi)
        } else {
            None
        }
    }

    fn del_output(&mut self, output: &Proxy<wl_output::WlOutput>) -> Option<i32> {
        let old_dpi = self.compute_hidpi_factor();
        self.monitors.retain(|m| !m.proxy.equals(output));
        let new_dpi = self.compute_hidpi_factor();
        if new_dpi != old_dpi {
            Some(new_dpi)
        } else {
            None
        }
    }
}
