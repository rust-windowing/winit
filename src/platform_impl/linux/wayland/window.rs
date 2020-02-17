use raw_window_handle::unix::WaylandHandle;
use std::{
    collections::VecDeque,
    mem::replace,
    sync::{Arc, Mutex, Weak},
};

use crate::{
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    monitor::MonitorHandle as RootMonitorHandle,
    platform_impl::{
        platform::wayland::event_loop::{available_monitors, primary_monitor},
        MonitorHandle as PlatformMonitorHandle,
        PlatformSpecificWindowBuilderAttributes as PlAttributes,
    },
    window::{CursorIcon, Fullscreen, WindowAttributes},
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

use super::{event_loop::CursorManager, make_wid, EventLoopWindowTarget, MonitorHandle, WindowId};

pub struct Window {
    surface: wl_surface::WlSurface,
    frame: Arc<Mutex<SWindow<ConceptFrame>>>,
    cursor_manager: Arc<Mutex<CursorManager>>,
    outputs: OutputMgr, // Access to info for all monitors
    size: Arc<Mutex<(u32, u32)>>,
    kill_switch: (Arc<Mutex<bool>>, Arc<Mutex<bool>>),
    display: Arc<Display>,
    need_frame_refresh: Arc<Mutex<bool>>,
    need_refresh: Arc<Mutex<bool>>,
    fullscreen: Arc<Mutex<bool>>,
    cursor_grab_changed: Arc<Mutex<Option<bool>>>, // Update grab state
    decorated: Arc<Mutex<bool>>,
}

#[derive(Clone, Copy, Debug)]
pub enum DecorationsAction {
    Hide,
    Show,
}

impl Window {
    pub fn new<T>(
        evlp: &EventLoopWindowTarget<T>,
        attributes: WindowAttributes,
        pl_attribs: PlAttributes,
    ) -> Result<Window, RootOsError> {
        // Create the surface first to get initial DPI
        let window_store = evlp.store.clone();
        let cursor_manager = evlp.cursor_manager.clone();
        let surface = evlp.env.create_surface(move |dpi, surface| {
            window_store.lock().unwrap().dpi_change(&surface, dpi);
            surface.set_buffer_scale(dpi);
        });

        let dpi = get_dpi_factor(&surface) as f64;
        let (width, height) = attributes
            .inner_size
            .map(|size| size.to_logical::<f64>(dpi).into())
            .unwrap_or((800, 600));

        // Create the window
        let size = Arc::new(Mutex::new((width, height)));
        let fullscreen = Arc::new(Mutex::new(false));

        let window_store = evlp.store.clone();

        let decorated = Arc::new(Mutex::new(attributes.decorations));
        let pending_decorations_action = Arc::new(Mutex::new(None));

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
                            {
                                // Get whether we're in fullscreen
                                let mut fullscreen = window.fullscreen.lock().unwrap();
                                // Fullscreen state was changed, so update decorations
                                if *fullscreen != is_fullscreen {
                                    let decorated = { *window.decorated.lock().unwrap() };
                                    if decorated {
                                        *window.pending_decorations_action.lock().unwrap() =
                                            if is_fullscreen {
                                                Some(DecorationsAction::Hide)
                                            } else {
                                                Some(DecorationsAction::Show)
                                            };
                                    }
                                }
                                *fullscreen = is_fullscreen;
                            }
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
        match attributes.fullscreen {
            Some(Fullscreen::Exclusive(_)) => {
                panic!("Wayland doesn't support exclusive fullscreen")
            }
            Some(Fullscreen::Borderless(RootMonitorHandle {
                inner: PlatformMonitorHandle::Wayland(ref monitor_id),
            })) => frame.set_fullscreen(Some(&monitor_id.proxy)),
            Some(Fullscreen::Borderless(_)) => unreachable!(),
            None => {
                if attributes.maximized {
                    frame.set_maximized();
                }
            }
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
                .map(|size| size.to_logical::<f64>(dpi).into()),
        );
        frame.set_max_size(
            attributes
                .max_inner_size
                .map(|size| size.to_logical::<f64>(dpi).into()),
        );

        let kill_switch = Arc::new(Mutex::new(false));
        let need_frame_refresh = Arc::new(Mutex::new(true));
        let frame = Arc::new(Mutex::new(frame));
        let need_refresh = Arc::new(Mutex::new(true));
        let cursor_grab_changed = Arc::new(Mutex::new(None));

        evlp.store.lock().unwrap().windows.push(InternalWindow {
            closed: false,
            newsize: None,
            size: size.clone(),
            need_refresh: need_refresh.clone(),
            fullscreen: fullscreen.clone(),
            cursor_grab_changed: cursor_grab_changed.clone(),
            need_frame_refresh: need_frame_refresh.clone(),
            surface: surface.clone(),
            kill_switch: kill_switch.clone(),
            frame: Arc::downgrade(&frame),
            current_dpi: 1,
            new_dpi: None,
            decorated: decorated.clone(),
            pending_decorations_action: pending_decorations_action.clone(),
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
            cursor_manager,
            fullscreen,
            cursor_grab_changed,
            decorated,
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
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn set_outer_position(&self, _pos: Position) {
        // Not possible with wayland
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let dpi = self.scale_factor() as f64;
        let size = LogicalSize::<f64>::from(*self.size.lock().unwrap());
        size.to_physical(dpi)
    }

    pub fn request_redraw(&self) {
        *self.need_refresh.lock().unwrap() = true;
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        let dpi = self.scale_factor() as f64;
        let (w, h) = self.size.lock().unwrap().clone();
        // let (w, h) = super::wayland_window::add_borders(w as i32, h as i32);
        let size = LogicalSize::<f64>::from((w, h));
        size.to_physical(dpi)
    }

    #[inline]
    // NOTE: This will only resize the borders, the contents must be updated by the user
    pub fn set_inner_size(&self, size: Size) {
        let dpi = self.scale_factor() as f64;
        let (w, h) = size.to_logical::<u32>(dpi).into();
        self.frame.lock().unwrap().resize(w, h);
        *(self.size.lock().unwrap()) = (w, h);
    }

    #[inline]
    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        let dpi = self.scale_factor() as f64;
        self.frame
            .lock()
            .unwrap()
            .set_min_size(dimensions.map(|dim| dim.to_logical::<f64>(dpi).into()));
    }

    #[inline]
    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        let dpi = self.scale_factor() as f64;
        self.frame
            .lock()
            .unwrap()
            .set_max_size(dimensions.map(|dim| dim.to_logical::<f64>(dpi).into()));
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        self.frame.lock().unwrap().set_resizable(resizable);
    }

    #[inline]
    pub fn scale_factor(&self) -> i32 {
        get_dpi_factor(&self.surface)
    }

    pub fn set_decorations(&self, decorate: bool) {
        *(self.decorated.lock().unwrap()) = decorate;
        self.frame.lock().unwrap().set_decorate(decorate);
        *(self.need_frame_refresh.lock().unwrap()) = true;
    }

    pub fn set_minimized(&self, minimized: bool) {
        // An app cannot un-minimize itself on Wayland
        if minimized {
            self.frame.lock().unwrap().set_minimized();
        }
    }

    pub fn set_maximized(&self, maximized: bool) {
        if maximized {
            self.frame.lock().unwrap().set_maximized();
        } else {
            self.frame.lock().unwrap().unset_maximized();
        }
    }

    pub fn fullscreen(&self) -> Option<Fullscreen> {
        if *(self.fullscreen.lock().unwrap()) {
            Some(Fullscreen::Borderless(RootMonitorHandle {
                inner: PlatformMonitorHandle::Wayland(self.current_monitor()),
            }))
        } else {
            None
        }
    }

    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        match fullscreen {
            Some(Fullscreen::Exclusive(_)) => {
                panic!("Wayland doesn't support exclusive fullscreen")
            }
            Some(Fullscreen::Borderless(RootMonitorHandle {
                inner: PlatformMonitorHandle::Wayland(ref monitor_id),
            })) => {
                self.frame
                    .lock()
                    .unwrap()
                    .set_fullscreen(Some(&monitor_id.proxy));
            }
            Some(Fullscreen::Borderless(_)) => unreachable!(),
            None => self.frame.lock().unwrap().unset_fullscreen(),
        }
    }

    pub fn set_theme<T: Theme>(&self, theme: T) {
        self.frame.lock().unwrap().set_theme(theme)
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        let mut cursor_manager = self.cursor_manager.lock().unwrap();
        cursor_manager.set_cursor_icon(cursor);
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        let mut cursor_manager = self.cursor_manager.lock().unwrap();
        cursor_manager.set_cursor_visible(visible);
    }

    #[inline]
    pub fn set_cursor_grab(&self, grab: bool) -> Result<(), ExternalError> {
        *self.cursor_grab_changed.lock().unwrap() = Some(grab);
        Ok(())
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

    pub fn raw_window_handle(&self) -> WaylandHandle {
        WaylandHandle {
            surface: self.surface().as_ref().c_ptr() as *mut _,
            display: self.display().as_ref().c_ptr() as *mut _,
            ..WaylandHandle::empty()
        }
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
    // TODO: CONVERT TO LogicalSize<u32>s
    newsize: Option<(u32, u32)>,
    size: Arc<Mutex<(u32, u32)>>,
    need_refresh: Arc<Mutex<bool>>,
    fullscreen: Arc<Mutex<bool>>,
    need_frame_refresh: Arc<Mutex<bool>>,
    cursor_grab_changed: Arc<Mutex<Option<bool>>>,
    closed: bool,
    kill_switch: Arc<Mutex<bool>>,
    frame: Weak<Mutex<SWindow<ConceptFrame>>>,
    current_dpi: i32,
    new_dpi: Option<i32>,
    decorated: Arc<Mutex<bool>>,
    pending_decorations_action: Arc<Mutex<Option<DecorationsAction>>>,
}

pub struct WindowStore {
    windows: Vec<InternalWindow>,
}

pub struct WindowStoreForEach<'a> {
    pub newsize: Option<(u32, u32)>,
    pub size: &'a mut (u32, u32),
    pub prev_dpi: i32,
    pub new_dpi: Option<i32>,
    pub closed: bool,
    pub grab_cursor: Option<bool>,
    pub surface: &'a wl_surface::WlSurface,
    pub wid: WindowId,
    pub frame: Option<&'a mut SWindow<ConceptFrame>>,
    pub decorations_action: Option<DecorationsAction>,
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
        F: FnMut(WindowStoreForEach<'_>),
    {
        for window in &mut self.windows {
            let opt_arc = window.frame.upgrade();
            let mut opt_mutex_lock = opt_arc.as_ref().map(|m| m.lock().unwrap());
            let mut size = { *window.size.lock().unwrap() };
            let decorations_action = { window.pending_decorations_action.lock().unwrap().take() };
            f(WindowStoreForEach {
                newsize: window.newsize.take(),
                size: &mut size,
                prev_dpi: window.current_dpi,
                new_dpi: window.new_dpi,
                closed: window.closed,
                grab_cursor: window.cursor_grab_changed.lock().unwrap().take(),
                surface: &window.surface,
                wid: make_wid(&window.surface),
                frame: opt_mutex_lock.as_mut().map(|m| &mut **m),
                decorations_action,
            });
            *window.size.lock().unwrap() = size;
            if let Some(dpi) = window.new_dpi.take() {
                window.current_dpi = dpi;
            }
            // avoid re-spamming the event
            window.closed = false;
        }
    }

    pub fn for_each_redraw_trigger<F>(&mut self, mut f: F)
    where
        F: FnMut(bool, bool, WindowId, Option<&mut SWindow<ConceptFrame>>),
    {
        for window in &mut self.windows {
            let opt_arc = window.frame.upgrade();
            let mut opt_mutex_lock = opt_arc.as_ref().map(|m| m.lock().unwrap());
            f(
                replace(&mut *window.need_refresh.lock().unwrap(), false),
                replace(&mut *window.need_frame_refresh.lock().unwrap(), false),
                make_wid(&window.surface),
                opt_mutex_lock.as_mut().map(|m| &mut **m),
            );
        }
    }
}
