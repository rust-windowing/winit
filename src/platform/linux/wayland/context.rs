use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};

use wayland_client::{EnvHandler, EnvNotify, default_connect, EventQueue, EventQueueHandle, Proxy, StateToken};
use wayland_client::protocol::{wl_compositor, wl_seat, wl_shell, wl_shm, wl_subcompositor,
                               wl_display, wl_registry, wl_output, wl_surface, wl_buffer};

use super::wayland_protocols::unstable::xdg_shell::client::zxdg_shell_v6;

use super::wayland_window::{self, Shell};

use super::tempfile;

pub struct WaylandContext {
    pub display: wl_display::WlDisplay,
    pub evq: Mutex<EventQueue>,
    env_token: StateToken<EnvHandler<InnerEnv>>,
    ctxt_token: StateToken<StateContext>,
}

impl WaylandContext {
    pub fn init() -> Option<WaylandContext> {
        // attempt to connect to the wayland server
        // this handles both "no libwayland" and "no compositor" cases
        let (display, mut event_queue) = match default_connect() {
            Ok(ret) => ret,
            Err(_) => return None
        };

        let registry = display.get_registry();
        let ctxt_token = event_queue.state().insert(
            StateContext::new(registry.clone().unwrap())
        );
        let env_token = EnvHandler::init_with_notify(
            &mut event_queue,
            &registry,
            env_notify(),
            ctxt_token.clone()
        );

        // two round trips to fully initialize
        event_queue.sync_roundtrip().expect("Wayland connection unexpectedly lost");
        event_queue.sync_roundtrip().expect("Wayland connection unexpectedly lost");

        event_queue.state().with_value(&ctxt_token, |proxy, ctxt| {
            ctxt.ensure_shell(proxy.get_mut(&env_token))
        });

        Some(WaylandContext {
            display: display,
            evq: Mutex::new(event_queue),
            env_token: env_token,
            ctxt_token: ctxt_token
        })
    }

    pub fn init_seat<F>(&mut self, f: F)
    where F: FnOnce(&mut EventQueueHandle, &wl_seat::WlSeat)
    {
        let guard = self.evq.get_mut().unwrap();
        if guard.state().get(&self.ctxt_token).seat.is_some() {
            // seat has already been init
            return;
        }

        // clone the token to make borrow checker happy
        let ctxt_token = self.ctxt_token.clone();
        let mut seat = guard.state().with_value(&self.env_token, |proxy, env| {
            let ctxt = proxy.get(&ctxt_token);
            for &(name, ref interface, _) in env.globals() {
                if interface == wl_seat::WlSeat::interface_name() {
                    return Some(ctxt.registry.bind::<wl_seat::WlSeat>(5, name));
                }
            }
            None
        });

        if let Some(seat) = seat {
            f(&mut *guard, &seat);
            guard.state().get_mut(&self.ctxt_token).seat = Some(seat)
        }
    }

    pub fn read_events(&self) {
        let evq_guard = self.evq.lock().unwrap();
        // read some events from the socket if some are waiting & queue is empty
        if let Some(guard) = evq_guard.prepare_read() {
            guard.read_events().expect("Wayland connection unexpectedly lost");
        }
    }

    pub fn dispatch_pending(&self) {
        let mut guard = self.evq.lock().unwrap();
        guard.dispatch_pending().expect("Wayland connection unexpectedly lost");
    }

    pub fn dispatch(&self) {
        let mut guard = self.evq.lock().unwrap();
        guard.dispatch().expect("Wayland connection unexpectedly lost");
    }

    pub fn flush(&self) {
        let _ = self.display.flush();
    }

    pub fn get_seat(&self) -> Option<wl_seat::WlSeat> {
        let mut guard = self.evq.lock().unwrap();
        guard.state()
             .get(&self.ctxt_token)
             .seat
             .as_ref()
             .and_then(|s| s.clone())
    }

    pub fn with_output_info<F, T>(&self, id: &MonitorId, f: F) -> Option<T>
    where F: FnOnce(&OutputInfo) -> T
    {
        let mut guard = self.evq.lock().unwrap();
        let ctxt = guard.state().get(&self.ctxt_token);
        for m in ctxt.monitors.iter().filter(|m| m.id == id.id) {
            return Some(f(m))
        }
        None
    }

    /// Creates a buffer of given size and assign it to the surface
    ///
    /// This buffer only contains white pixels, and is needed when using wl_shell
    /// to make sure the window actually exists and can receive events before the
    /// use starts its event loop
    fn blank_surface(&self, surface: &wl_surface::WlSurface, width: i32, height: i32) {
        let mut tmp = tempfile::tempfile().expect("Failed to create a tmpfile buffer.");
        for _ in 0..(width*height) {
            tmp.write_all(&[0xff,0xff,0xff,0xff]).unwrap();
        }
        tmp.flush().unwrap();
        let mut evq = self.evq.lock().unwrap();
        let pool = evq.state()
                      .get(&self.env_token)
                      .shm
                      .create_pool(tmp.as_raw_fd(), width*height*4);
        let buffer = pool.create_buffer(0, width, height, width, wl_shm::Format::Argb8888)
                         .expect("Pool cannot be already dead");
        surface.attach(Some(&buffer), 0, 0);
        surface.commit();
        // the buffer will keep the contents alive as needed
        pool.destroy();
        // register the buffer for freeing
        evq.register(&buffer, free_buffer(), Some(tmp));
    }

    /// Create a new window with given dimensions
    ///
    /// Grabs a lock on the event queue in the process
    pub fn create_window<ID: 'static>(&self, width: u32, height: u32, decorated: bool, implem: wayland_window::DecoratedSurfaceImplementation<ID>, idata: ID)
        -> (Arc<wl_surface::WlSurface>, wayland_window::DecoratedSurface, bool)
    {
        let (surface, decorated, xdg) = {
            let mut guard = self.evq.lock().unwrap();
            let env = guard.state().get(&self.env_token).clone_inner().unwrap();
            let (shell, xdg) = match guard.state().get(&self.ctxt_token).shell {
                Some(Shell::Wl(ref wl_shell)) => (Shell::Wl(wl_shell.clone().unwrap()), false),
                Some(Shell::Xdg(ref xdg_shell)) => (Shell::Xdg(xdg_shell.clone().unwrap()), true),
                None => unreachable!()
            };
            let seat = guard.state().get(&self.ctxt_token).seat.as_ref().and_then(|s| s.clone());
            let surface = Arc::new(env.compositor.create_surface());
            let decorated = wayland_window::init_decorated_surface(
                &mut guard,
                implem,
                idata,
                &*surface, width as i32, height as i32,
                &env.compositor,
                &env.subcompositor,
                &env.shm,
                &shell,
                seat,
                decorated
            ).expect("Failed to create a tmpfile buffer.");
            (surface, decorated, xdg)
        };

        if !xdg {
            // if using wl_shell, we need to draw something in order to kickstart
            // the event loop
            // if using xdg_shell, it is an error to do it now, and the events loop will not
            // be stuck. We cannot draw anything before having received an appropriate event
            // from the compositor
            self.blank_surface(&surface, width as i32, height as i32);
        }
        (surface, decorated, xdg)
    }
}

/*
 * Protocol handling
 */

wayland_env!(InnerEnv,
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    subcompositor: wl_subcompositor::WlSubcompositor
);

struct StateContext {
    registry: wl_registry::WlRegistry,
    seat: Option<wl_seat::WlSeat>,
    shell: Option<Shell>,
    monitors: Vec<OutputInfo>
}

impl StateContext {
    fn new(registry: wl_registry::WlRegistry) -> StateContext {
        StateContext {
            registry: registry,
            seat: None,
            shell: None,
            monitors: Vec::new()
        }
    }

    /// Ensures a shell is available
    ///
    /// If a shell is already bound, do nothing. Otherwise,
    /// try to bind wl_shell as a fallback. If this fails,
    /// panic, as this is a bug from the compositor.
    fn ensure_shell(&mut self, env: &mut EnvHandler<InnerEnv>) {
        if self.shell.is_some() {
            return;
        }
        // xdg_shell is not available, so initialize wl_shell
        for &(name, ref interface, _) in env.globals() {
            if interface == "wl_shell" {
                self.shell = Some(Shell::Wl(self.registry.bind::<wl_shell::WlShell>(1, name)));
                return;
            }
        }
        // This is a compositor bug, it _must_ at least support wl_shell
        panic!("Compositor didi not advertize xdg_shell not wl_shell.");
    }
}

fn env_notify() -> EnvNotify<StateToken<StateContext>> {
    EnvNotify {
        new_global: |evqh, token, registry, id, interface, version| {
            if interface == wl_output::WlOutput::interface_name() {
                // a new output is available
                let output = registry.bind::<wl_output::WlOutput>(1, id);
                evqh.register(&output, output_impl(), token.clone());
                evqh.state().get_mut(&token).monitors.push(OutputInfo::new(output, id));
            } else if interface == zxdg_shell_v6::ZxdgShellV6::interface_name() {
                // We have an xdg_shell, bind it
                let xdg_shell = registry.bind::<zxdg_shell_v6::ZxdgShellV6>(1, id);
                evqh.register(&xdg_shell, xdg_ping_implementation(), ());
                evqh.state().get_mut(&token).shell = Some(Shell::Xdg(xdg_shell));
            }
        },
        del_global: |evqh, token, _, id| {
            // maybe this was a monitor, cleanup
            evqh.state().get_mut(&token).monitors.retain(|m| m.id != id);
        },
        ready: |_, _, _| {}
    }
}

fn xdg_ping_implementation() -> zxdg_shell_v6::Implementation<()> {
    zxdg_shell_v6::Implementation {
        ping: |_, _, shell, serial| {
            shell.pong(serial);
        }
    }
}

fn free_buffer() -> wl_buffer::Implementation<Option<File>> {
    wl_buffer::Implementation {
        release: |_, data, buffer| {
            buffer.destroy();
            *data = None;
        }
    }
}

/*
 * Monitor stuff
 */

fn output_impl() -> wl_output::Implementation<StateToken<StateContext>> {
    wl_output::Implementation {
        geometry: |evqh, token, output, x, y, _, _, _, make, model, _| {
            let ctxt = evqh.state().get_mut(token);
            if let Some(info) = ctxt.monitors.iter_mut().find(|info| info.output.equals(output)) {
                info.pix_pos = (x, y);
                info.name = format!("{} - {}", make, model);
            }
        },
        mode: |evqh, token, output, flags, w, h, _refresh| {
            if flags.contains(wl_output::Mode::Current) {
                let ctxt = evqh.state().get_mut(token);
                if let Some(info) = ctxt.monitors.iter_mut().find(|info| info.output.equals(output)) {
                    info.pix_size = (w as u32, h as u32);
                }
            }
        },
        done: |_, _, _| {},
        scale: |evqh, token, output, scale| {
            let ctxt = evqh.state().get_mut(token);
            if let Some(info) = ctxt.monitors.iter_mut().find(|info| info.output.equals(output)) {
                info.scale = scale as f32;
            }
        }
    }
}

pub struct OutputInfo {
    pub output: wl_output::WlOutput,
    pub id: u32,
    pub scale: f32,
    pub pix_size: (u32, u32),
    pub pix_pos: (i32, i32),
    pub name: String
}

impl OutputInfo {
    fn new(output: wl_output::WlOutput, id: u32) -> OutputInfo {
        OutputInfo {
            output: output,
            id: id,
            scale: 1.0,
            pix_size: (0, 0),
            pix_pos: (0, 0),
            name: "".into()
        }
    }
}

pub fn get_primary_monitor(ctxt: &Arc<WaylandContext>) -> MonitorId {
    let mut guard = ctxt.evq.lock().unwrap();
    let state = guard.state();
    let state_ctxt = state.get(&ctxt.ctxt_token);
    if let Some(ref monitor) = state_ctxt.monitors.iter().next() {
        MonitorId {
            id: monitor.id,
            ctxt: ctxt.clone()
        }
    } else {
        panic!("No monitor is available.")
    }
}

pub fn get_available_monitors(ctxt: &Arc<WaylandContext>) -> VecDeque<MonitorId> {
    let mut guard = ctxt.evq.lock().unwrap();
    let state = guard.state();
    let state_ctxt = state.get(&ctxt.ctxt_token);
    state_ctxt.monitors.iter()
       .map(|m| MonitorId { id: m.id, ctxt: ctxt.clone() })
       .collect()
}

#[derive(Clone)]
pub struct MonitorId {
    id: u32,
    ctxt: Arc<WaylandContext>
}

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        self.ctxt.with_output_info(self, |info| info.name.clone())
    }

    #[inline]
    pub fn get_native_identifier(&self) -> u32 {
        self.id
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        self.ctxt.with_output_info(self, |info| info.pix_size)
                 .unwrap_or((0,0))
    }

    pub fn get_position(&self) -> (i32, i32) {
        self.ctxt.with_output_info(self, |info| info.pix_pos)
                 .unwrap_or((0,0))
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f32 {
        unimplemented!()
    }
}
