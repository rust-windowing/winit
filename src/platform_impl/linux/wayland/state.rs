use std::cell::RefCell;
use std::error::Error;
use std::sync::{Arc, Mutex};

use fnv::FnvHashMap;

use sctk::reexports::calloop::LoopHandle;
use sctk::reexports::client::backend::ObjectId;
use sctk::reexports::client::globals::GlobalList;
use sctk::reexports::client::protocol::wl_output::WlOutput;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{Connection, Proxy, QueueHandle};

use sctk::compositor::{CompositorHandler, CompositorState};
use sctk::output::{OutputHandler, OutputState};
use sctk::registry::{ProvidesRegistryState, RegistryState};
use sctk::seat::pointer::ThemedPointer;
use sctk::seat::SeatState;
use sctk::shell::xdg::window::{Window, WindowConfigure, WindowHandler};
use sctk::shell::xdg::XdgShell;
use sctk::shell::WaylandSurface;
use sctk::shm::{Shm, ShmHandler};
use sctk::subcompositor::SubcompositorState;

use crate::dpi::LogicalSize;

use super::event_loop::sink::EventSink;
use super::output::MonitorHandle;
use super::seat::{
    PointerConstraintsState, RelativePointerState, TextInputState, WinitPointerData,
    WinitPointerDataExt, WinitSeatState,
};
use super::types::wp_fractional_scaling::FractionalScalingManager;
use super::types::wp_viewporter::ViewporterState;
use super::types::xdg_activation::XdgActivationState;
use super::window::{WindowRequests, WindowState};
use super::WindowId;

/// Winit's Wayland state.
pub struct WinitState {
    /// The WlRegistry.
    pub registry_state: RegistryState,

    /// The state of the WlOutput handling.
    pub output_state: OutputState,

    /// The compositor state which is used to create new windows and regions.
    pub compositor_state: Arc<CompositorState>,

    /// The state of the subcompositor.
    pub subcompositor_state: Arc<SubcompositorState>,

    /// The seat state responsible for all sorts of input.
    pub seat_state: SeatState,

    /// The shm for software buffers, such as cursors.
    pub shm: Shm,

    /// The XDG shell that is used for widnows.
    pub xdg_shell: XdgShell,

    /// The currently present windows.
    pub windows: RefCell<FnvHashMap<WindowId, Arc<Mutex<WindowState>>>>,

    /// The requests from the `Window` to EventLoop, such as close operations and redraw requests.
    pub window_requests: RefCell<FnvHashMap<WindowId, Arc<WindowRequests>>>,

    /// The events that were generated directly from the window.
    pub window_events_sink: Arc<Mutex<EventSink>>,

    /// The update for the `windows` comming from the compositor.
    pub window_compositor_updates: Vec<WindowCompositorUpdate>,

    /// Currently handled seats.
    pub seats: FnvHashMap<ObjectId, WinitSeatState>,

    /// Currently present cursor surfaces.
    pub pointer_surfaces: FnvHashMap<ObjectId, Arc<ThemedPointer<WinitPointerData>>>,

    /// The state of the text input on the client.
    pub text_input_state: Option<TextInputState>,

    /// Observed monitors.
    pub monitors: Arc<Mutex<Vec<MonitorHandle>>>,

    /// Sink to accumulate window events from the compositor, which is latter dispatched in
    /// event loop run.
    pub events_sink: EventSink,

    /// Xdg activation.
    pub xdg_activation: Option<XdgActivationState>,

    /// Relative pointer.
    pub relative_pointer: Option<RelativePointerState>,

    /// Pointer constraints to handle pointer locking and confining.
    pub pointer_constraints: Option<Arc<PointerConstraintsState>>,

    /// Viewporter state on the given window.
    pub viewporter_state: Option<ViewporterState>,

    /// Fractional scaling manager.
    pub fractional_scaling_manager: Option<FractionalScalingManager>,

    /// Loop handle to re-register event sources, such as keyboard repeat.
    pub loop_handle: LoopHandle<'static, Self>,
}

impl WinitState {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<Self>,
        loop_handle: LoopHandle<'static, WinitState>,
    ) -> Result<Self, Box<dyn Error>> {
        let registry_state = RegistryState::new(globals);
        let compositor_state = CompositorState::bind(globals, queue_handle)?;
        let subcompositor_state = SubcompositorState::bind(
            compositor_state.wl_compositor().clone(),
            globals,
            queue_handle,
        )?;

        let output_state = OutputState::new(globals, queue_handle);
        let monitors = output_state.outputs().map(MonitorHandle::new).collect();

        let seat_state = SeatState::new(globals, queue_handle);

        let mut seats = FnvHashMap::default();
        for seat in seat_state.seats() {
            seats.insert(seat.id(), WinitSeatState::new());
        }

        let (viewporter_state, fractional_scaling_manager) =
            if let Ok(fsm) = FractionalScalingManager::new(globals, queue_handle) {
                (ViewporterState::new(globals, queue_handle).ok(), Some(fsm))
            } else {
                (None, None)
            };

        Ok(Self {
            registry_state,
            compositor_state: Arc::new(compositor_state),
            subcompositor_state: Arc::new(subcompositor_state),
            output_state,
            seat_state,
            shm: Shm::bind(globals, queue_handle)?,

            xdg_shell: XdgShell::bind(globals, queue_handle)?,
            xdg_activation: XdgActivationState::bind(globals, queue_handle).ok(),

            windows: Default::default(),
            window_requests: Default::default(),
            window_compositor_updates: Vec::new(),
            window_events_sink: Default::default(),
            viewporter_state,
            fractional_scaling_manager,

            seats,
            text_input_state: TextInputState::new(globals, queue_handle).ok(),

            relative_pointer: RelativePointerState::new(globals, queue_handle).ok(),
            pointer_constraints: PointerConstraintsState::new(globals, queue_handle)
                .map(Arc::new)
                .ok(),
            pointer_surfaces: Default::default(),

            monitors: Arc::new(Mutex::new(monitors)),
            events_sink: EventSink::new(),
            loop_handle,
        })
    }

    pub fn scale_factor_changed(
        &mut self,
        surface: &WlSurface,
        scale_factor: f64,
        is_legacy: bool,
    ) {
        // Check if the cursor surface.
        let window_id = super::make_wid(surface);

        if let Some(window) = self.windows.get_mut().get(&window_id) {
            // Don't update the scaling factor, when legacy method is used.
            if is_legacy && self.fractional_scaling_manager.is_some() {
                return;
            }

            // The scale factor change is for the window.
            let pos = if let Some(pos) = self
                .window_compositor_updates
                .iter()
                .position(|update| update.window_id == window_id)
            {
                pos
            } else {
                self.window_compositor_updates
                    .push(WindowCompositorUpdate::new(window_id));
                self.window_compositor_updates.len() - 1
            };

            // Update the scale factor right away.
            window.lock().unwrap().set_scale_factor(scale_factor);
            self.window_compositor_updates[pos].scale_factor = Some(scale_factor);
        } else if let Some(pointer) = self.pointer_surfaces.get(&surface.id()) {
            // Get the window, where the pointer resides right now.
            let focused_window = match pointer.pointer().winit_data().focused_window() {
                Some(focused_window) => focused_window,
                None => return,
            };

            if let Some(window_state) = self.windows.get_mut().get(&focused_window) {
                window_state.lock().unwrap().reload_cursor_style()
            }
        }
    }

    pub fn queue_close(updates: &mut Vec<WindowCompositorUpdate>, window_id: WindowId) {
        let pos = if let Some(pos) = updates
            .iter()
            .position(|update| update.window_id == window_id)
        {
            pos
        } else {
            updates.push(WindowCompositorUpdate::new(window_id));
            updates.len() - 1
        };

        updates[pos].close_window = true;
    }
}

impl ShmHandler for WinitState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl WindowHandler for WinitState {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window) {
        let window_id = super::make_wid(window.wl_surface());
        Self::queue_close(&mut self.window_compositor_updates, window_id);
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        let window_id = super::make_wid(window.wl_surface());

        let pos = if let Some(pos) = self
            .window_compositor_updates
            .iter()
            .position(|update| update.window_id == window_id)
        {
            pos
        } else {
            self.window_compositor_updates
                .push(WindowCompositorUpdate::new(window_id));
            self.window_compositor_updates.len() - 1
        };

        // Populate the configure to the window.
        //
        // XXX the size on the window will be updated right before dispatching the size to the user.
        let new_size = self
            .windows
            .get_mut()
            .get_mut(&window_id)
            .expect("got configure for dead window.")
            .lock()
            .unwrap()
            .configure(configure, &self.shm, &self.subcompositor_state);

        self.window_compositor_updates[pos].size = Some(new_size);
    }
}

impl OutputHandler for WinitState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        self.monitors
            .lock()
            .unwrap()
            .push(MonitorHandle::new(output));
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, updated: WlOutput) {
        let mut monitors = self.monitors.lock().unwrap();
        let updated = MonitorHandle::new(updated);
        if let Some(pos) = monitors.iter().position(|output| output == &updated) {
            monitors[pos] = updated
        } else {
            monitors.push(updated)
        }
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, removed: WlOutput) {
        let mut monitors = self.monitors.lock().unwrap();
        let removed = MonitorHandle::new(removed);
        if let Some(pos) = monitors.iter().position(|output| output == &removed) {
            monitors.remove(pos);
        }
    }
}

impl CompositorHandler for WinitState {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &WlSurface,
        scale_factor: i32,
    ) {
        self.scale_factor_changed(surface, scale_factor as f64, true)
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: u32) {}
}

impl ProvidesRegistryState for WinitState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    sctk::registry_handlers![OutputState, SeatState];
}

// The window update comming from the compositor.
#[derive(Debug, Clone, Copy)]
pub struct WindowCompositorUpdate {
    /// The id of the window this updates belongs to.
    pub window_id: WindowId,

    /// New window size.
    pub size: Option<LogicalSize<u32>>,

    /// New scale factor.
    pub scale_factor: Option<f64>,

    /// Close the window.
    pub close_window: bool,
}

impl WindowCompositorUpdate {
    fn new(window_id: WindowId) -> Self {
        Self {
            window_id,
            size: None,
            scale_factor: None,
            close_window: false,
        }
    }
}

sctk::delegate_subcompositor!(WinitState);
sctk::delegate_compositor!(WinitState);
sctk::delegate_output!(WinitState);
sctk::delegate_registry!(WinitState);
sctk::delegate_shm!(WinitState);
sctk::delegate_xdg_shell!(WinitState);
sctk::delegate_xdg_window!(WinitState);
