use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, Weak},
};

use crate::{
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    monitor::MonitorHandle as RootMonitorHandle,
    platform_impl::{
        MonitorHandle as PlatformMonitorHandle,
        PlatformSpecificWindowBuilderAttributes as PlAttributes,
    },
    window::{CursorIcon, WindowAttributes},
};

use smithay_client_toolkit::{
    output::OutputMgr,
    reexports::client::{
        protocol::{wl_seat, wl_surface},
        Display,
    },
    surface::{get_dpi_factor, get_outputs},
    window::{ConceptFrame, Event as WEvent, State as WState, Theme, Window as SWindow},
};

use super::{make_wid, EventLoopWindowTarget, MonitorHandle, WindowId};
use crate::platform_impl::platform::wayland::event_loop::{available_monitors, primary_monitor};

pub struct Window {
    surface: wl_surface::WlSurface,
    frame: Arc<Mutex<SWindow<ConceptFrame>>>,
    outputs: OutputMgr, // Access to info for all monitors
    size: Arc<Mutex<(u32, u32)>>,
    kill_switch: (Arc<Mutex<bool>>, Arc<Mutex<bool>>),
    display: Arc<Display>,
    need_frame_refresh: Arc<Mutex<bool>>,
    need_refresh: Arc<Mutex<bool>>,
    fullscreen: Arc<Mutex<bool>>,
}

impl Window {
    pub fn new<T>(
        evlp: &EventLoopWindowTarget<T>,
        attributes: WindowAttributes,
        pl_attribs: PlAttributes,
    ) -> Result<Window, RootOsError> {
        // Create the surface first to get initial DPI
        let window_store = evlp.store.clone();
        let surface = evlp.env.create_surface(move |dpi, surface| {
            window_store.lock().unwrap().dpi_change(&surface, dpi);
            surface.set_buffer_scale(dpi);
        });

        let dpi = get_dpi_factor(&surface) as f64;
        let (width, height) = attributes
            .inner_size
            .map(|size| size.to_logical(dpi).into())
            .unwrap_or((800, 600));

        // Create the window
        let size = Arc::new(Mutex::new((width, height)));
        let fullscreen = Arc::new(Mutex::new(false));

        let window_store = evlp.store.clone();

        let my_surface = surface.clone();
        let mut frame = SWindow::<ConceptFrame>::init_from_env(
            &evlp.env,
            surface.clone(),
            (width, height),
            move |event| match event {
                WEvent::Configure { new_size, states } => {
                    let mut store = window_store.lock().unwrap();
                    let is_fullscreen = states.contains(&WState::Fullscreen);

                    for window in &mut store.windows {
                        if window.surface.as_ref().equals(&my_surface.as_ref()) {
                            window.newsize = new_size;
                            *(window.need_refresh.lock().unwrap()) = true;
                            *(window.fullscreen.lock().unwrap()) = is_fullscreen;
                            *(window.need_frame_refresh.lock().unwrap()) = true;
                            return;
                        }
                    }
                }
                WEvent::Refresh => {
                    let store = window_store.lock().unwrap();
                    for window in &store.windows {
                        if window.surface.as_ref().equals(&my_surface.as_ref()) {
                            *(window.need_frame_refresh.lock().unwrap()) = true;
                            return;
                        }
                    }
                }
                WEvent::Close => {
                    let mut store = window_store.lock().unwrap();
                    for window in &mut store.windows {
                        if window.surface.as_ref().equals(&my_surface.as_ref()) {
                            window.closed = true;
                            return;
                        }
                    }
                }
            },
        )
        .unwrap();

        if let Some(app_id) = pl_attribs.app_id {
            frame.set_app_id(app_id);
        }

        for &(_, ref seat) in evlp.seats.lock().unwrap().iter() {
            frame.new_seat(seat);
        }

        // Check for fullscreen requirements
        if let Some(RootMonitorHandle {
            inner: PlatformMonitorHandle::Wayland(ref monitor_id),
        }) = attributes.fullscreen
        {
            frame.set_fullscreen(Some(&monitor_id.proxy));
        } else if attributes.maximized {
            frame.set_maximized();
        }

        frame.set_resizable(attributes.resizable);

        // set decorations
        frame.set_decorate(attributes.decorations);

        // set title
        frame.set_title(attributes.title);

        // min-max dimensions
        frame.set_min_size(
            attributes
                .min_inner_size
                .map(|size| size.to_logical(dpi).into()),
        );
        frame.set_max_size(
            attributes
                .max_inner_size
                .map(|size| size.to_logical(dpi).into()),
        );

        let kill_switch = Arc::new(Mutex::new(false));
        let need_frame_refresh = Arc::new(Mutex::new(true));
        let frame = Arc::new(Mutex::new(frame));
        let need_refresh = Arc::new(Mutex::new(true));

        evlp.store.lock().unwrap().windows.push(InternalWindow {
            closed: false,
            newsize: None,
            size: size.clone(),
            need_refresh: need_refresh.clone(),
            fullscreen: fullscreen.clone(),
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
            surface,
            frame,
            outputs: evlp.env.outputs.clone(),
            size,
            kill_switch: (kill_switch, evlp.cleanup_needed.clone()),
            need_frame_refresh,
            need_refresh,
            fullscreen,
        })
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        make_wid(&self.surface)
    }

    pub fn set_title(&self, title: &str) {
        self.frame.lock().unwrap().set_title(title.into());
    }

    pub fn set_visible(&self, _visible: bool) {
        // TODO
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn set_outer_position(&self, _pos: Position) {
        // Not possible with wayland
    }

    pub fn inner_size(&self) -> PhysicalSize {
        let dpi = self.hidpi_factor() as f64;
        let size = LogicalSize::from(*self.size.lock().unwrap());
        size.to_physical(dpi)
    }

    pub fn request_redraw(&self) {
        *self.need_refresh.lock().unwrap() = true;
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize {
        let dpi = self.hidpi_factor() as f64;
        let (w, h) = self.size.lock().unwrap().clone();
        // let (w, h) = super::wayland_window::add_borders(w as i32, h as i32);
        let size = LogicalSize::from((w, h));
        size.to_physical(dpi)
    }

    #[inline]
    // NOTE: This will only resize the borders, the contents must be updated by the user
    pub fn set_inner_size(&self, size: Size) {
        let dpi = self.hidpi_factor() as f64;
        let (w, h) = size.to_logical(dpi).into();
        self.frame.lock().unwrap().resize(w, h);
        *(self.size.lock().unwrap()) = (w, h);
    }

    #[inline]
    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        let dpi = self.hidpi_factor() as f64;
        self.frame
            .lock()
            .unwrap()
            .set_min_size(dimensions.map(|dim| dim.to_logical(dpi).into()));
    }

    #[inline]
    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        let dpi = self.hidpi_factor() as f64;
        self.frame
            .lock()
            .unwrap()
            .set_max_size(dimensions.map(|dim| dim.to_logical(dpi).into()));
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        self.frame.lock().unwrap().set_resizable(resizable);
    }

    #[inline]
    pub fn hidpi_factor(&self) -> i32 {
        get_dpi_factor(&self.surface)
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

    pub fn fullscreen(&self) -> Option<MonitorHandle> {
        if *(self.fullscreen.lock().unwrap()) {
            Some(self.current_monitor())
        } else {
            None
        }
    }

    pub fn set_fullscreen(&self, monitor: Option<RootMonitorHandle>) {
        if let Some(RootMonitorHandle {
            inner: PlatformMonitorHandle::Wayland(ref monitor_id),
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

    pub fn set_theme<T: Theme>(&self, theme: T) {
        self.frame.lock().unwrap().set_theme(theme)
    }

    #[inline]
    pub fn set_cursor_icon(&self, _cursor: CursorIcon) {
        // TODO
    }

    #[inline]
    pub fn set_cursor_visible(&self, _visible: bool) {
        // TODO: This isn't possible on Wayland yet
    }

    #[inline]
    pub fn set_cursor_grab(&self, _grab: bool) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn set_cursor_position(&self, _pos: Position) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn display(&self) -> &Display {
        &*self.display
    }

    pub fn surface(&self) -> &wl_surface::WlSurface {
        &self.surface
    }

    pub fn current_monitor(&self) -> MonitorHandle {
        let output = get_outputs(&self.surface).last().unwrap().clone();
        MonitorHandle {
            proxy: output,
            mgr: self.outputs.clone(),
        }
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        available_monitors(&self.outputs)
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        primary_monitor(&self.outputs)
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
    surface: wl_surface::WlSurface,
    newsize: Option<(u32, u32)>,
    size: Arc<Mutex<(u32, u32)>>,
    need_refresh: Arc<Mutex<bool>>,
    fullscreen: Arc<Mutex<bool>>,
    need_frame_refresh: Arc<Mutex<bool>>,
    closed: bool,
    kill_switch: Arc<Mutex<bool>>,
    frame: Weak<Mutex<SWindow<ConceptFrame>>>,
    current_dpi: i32,
    new_dpi: Option<i32>,
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

    pub fn find_wid(&self, surface: &wl_surface::WlSurface) -> Option<WindowId> {
        for window in &self.windows {
            if surface.as_ref().equals(&window.surface.as_ref()) {
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

    pub fn new_seat(&self, seat: &wl_seat::WlSeat) {
        for window in &self.windows {
            if let Some(w) = window.frame.upgrade() {
                w.lock().unwrap().new_seat(seat);
            }
        }
    }

    fn dpi_change(&mut self, surface: &wl_surface::WlSurface, new: i32) {
        for window in &mut self.windows {
            if surface.as_ref().equals(&window.surface.as_ref()) {
                window.new_dpi = Some(new);
            }
        }
    }

    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(
            Option<(u32, u32)>,
            &mut (u32, u32),
            i32,
            Option<i32>,
            bool,
            bool,
            bool,
            WindowId,
            Option<&mut SWindow<ConceptFrame>>,
        ),
    {
        for window in &mut self.windows {
            let opt_arc = window.frame.upgrade();
            let mut opt_mutex_lock = opt_arc.as_ref().map(|m| m.lock().unwrap());
            f(
                window.newsize.take(),
                &mut *(window.size.lock().unwrap()),
                window.current_dpi,
                window.new_dpi,
                ::std::mem::replace(&mut *window.need_refresh.lock().unwrap(), false),
                ::std::mem::replace(&mut *window.need_frame_refresh.lock().unwrap(), false),
                window.closed,
                make_wid(&window.surface),
                opt_mutex_lock.as_mut().map(|m| &mut **m),
            );
            if let Some(dpi) = window.new_dpi.take() {
                window.current_dpi = dpi;
            }
            // avoid re-spamming the event
            window.closed = false;
        }
    }
}
