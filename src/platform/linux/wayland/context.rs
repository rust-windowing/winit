use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};

use wayland_client::{EnvHandler, default_connect, EventQueue, EventQueueHandle, Init, Proxy};
use wayland_client::protocol::{wl_compositor, wl_seat, wl_shell, wl_shm, wl_subcompositor,
                               wl_display, wl_registry, wl_output, wl_surface, wl_buffer};

use super::wayland_protocols::unstable::xdg_shell::client::zxdg_shell_v6;

use super::{wayland_window, tempfile};

use super::wayland_window::Shell;

/*
 * Registry and globals handling
 */

wayland_env!(InnerEnv,
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    subcompositor: wl_subcompositor::WlSubcompositor
);

struct WaylandEnv {
    registry: wl_registry::WlRegistry,
    inner: EnvHandler<InnerEnv>,
    shell: Option<wayland_window::Shell>,
    monitors: Vec<OutputInfo>,
    my_id: usize,
}

struct OutputInfo {
    output: wl_output::WlOutput,
    id: u32,
    scale: f32,
    pix_size: (u32, u32),
    name: String
}

impl OutputInfo {
    fn new(output: wl_output::WlOutput, id: u32) -> OutputInfo {
        OutputInfo {
            output: output,
            id: id,
            scale: 1.0,
            pix_size: (0, 0),
            name: "".into()
        }
    }
}

impl WaylandEnv {
    fn new(registry: wl_registry::WlRegistry) -> WaylandEnv {
        WaylandEnv {
            registry: registry,
            inner: EnvHandler::new(),
            shell: None,
            monitors: Vec::new(),
            my_id: 0,
        }
    }

    fn get_seat(&self) -> Option<wl_seat::WlSeat> {
        for &(name, ref interface, version) in self.inner.globals() {
            if interface == "wl_seat" {
                if version < 5 {
                    panic!("Winit requires at least version 5 of the wl_seat global.");
                }
                let seat = self.registry.bind::<wl_seat::WlSeat>(5, name);
                return Some(seat)
            }
        }
        None
    }

    fn ensure_shell(&mut self) -> bool {
        if self.shell.is_some() {
            return true;
        }
        // xdg_shell is not available, so initialize wl_shell
        for &(name, ref interface, _) in self.inner.globals() {
            if interface == "wl_shell" {
                self.shell = Some(Shell::Wl(self.registry.bind::<wl_shell::WlShell>(1, name)));
                return true;
            }
        }
        return false;
    }

    fn get_shell(&self) -> &Shell {
        self.shell.as_ref().expect("Shell was not properly initialized")
    }
}

impl Init for WaylandEnv {
    fn init(&mut self, evqh: &mut EventQueueHandle, index: usize) {
        evqh.register::<_, WaylandEnv>(&self.registry, index);
        self.my_id = index
    }
}

impl wl_registry::Handler for WaylandEnv {
    fn global(&mut self,
              evqh: &mut EventQueueHandle,
              registry: &wl_registry::WlRegistry,
              name: u32,
              interface: String,
              version: u32)
    {
        if interface == wl_output::WlOutput::interface_name() {
            // intercept outputs
            // this "expect" cannot trigger (see https://github.com/vberger/wayland-client-rs/issues/69)
            let output = self.registry.bind::<wl_output::WlOutput>(1, name);
            evqh.register::<_, WaylandEnv>(&output, self.my_id);
            self.monitors.push(OutputInfo::new(output, name));
        } else if interface == zxdg_shell_v6::ZxdgShellV6::interface_name() {
            let xdg_shell = self.registry.bind::<zxdg_shell_v6::ZxdgShellV6>(1, name);
			let xdg_ping_hid = evqh.add_handler(XdgShellPingHandler);
            evqh.register::<_, XdgShellPingHandler>(&xdg_shell, xdg_ping_hid);
            self.shell = Some(Shell::Xdg(xdg_shell));
        }

        self.inner.global(evqh, registry, name, interface, version);
    }

    fn global_remove(&mut self,
                     evqh: &mut EventQueueHandle,
                     registry: &wl_registry::WlRegistry,
                     name: u32)
    {
        // prune old monitors
        self.monitors.retain(|m| m.id != name);
        self.inner.global_remove(evqh, registry, name);
    }
}

struct XdgShellPingHandler;

impl zxdg_shell_v6::Handler for XdgShellPingHandler {
    fn ping(&mut self, _: &mut EventQueueHandle, proxy: &zxdg_shell_v6::ZxdgShellV6, serial: u32) {
        proxy.pong(serial);
    }
}

declare_handler!(XdgShellPingHandler, zxdg_shell_v6::Handler, zxdg_shell_v6::ZxdgShellV6);

declare_handler!(WaylandEnv, wl_registry::Handler, wl_registry::WlRegistry);

impl wl_output::Handler for WaylandEnv {
    fn geometry(&mut self,
                _: &mut EventQueueHandle,
                proxy: &wl_output::WlOutput,
                _x: i32, _y: i32,
                _physical_width: i32, _physical_height: i32,
                _subpixel: wl_output::Subpixel,
                make: String, model: String,
                _transform: wl_output::Transform)
    {
        for m in self.monitors.iter_mut().filter(|m| m.output.equals(proxy)) {
            m.name = format!("{} ({})", model, make);
            break;
        }
    }
    fn mode(&mut self,
            _: &mut EventQueueHandle,
            proxy: &wl_output::WlOutput,
            flags: wl_output::Mode,
            width: i32, height: i32,
            _refresh: i32)
    {
        if flags.contains(wl_output::Current) {
            for m in self.monitors.iter_mut().filter(|m| m.output.equals(proxy)) {
                m.pix_size = (width as u32, height as u32);
                break;
            }
        }
    }
    fn scale(&mut self,
             _: &mut EventQueueHandle,
             proxy: &wl_output::WlOutput,
             factor: i32)
    {
        for m in self.monitors.iter_mut().filter(|m| m.output.equals(proxy)) {
            m.scale = factor as f32;
            break;
        }
    }
}

declare_handler!(WaylandEnv, wl_output::Handler, wl_output::WlOutput);

/*
 * Main context struct
 */

pub struct WaylandContext {
    pub display: wl_display::WlDisplay,
    evq: Mutex<EventQueue>,
    env_id: usize,
}

impl WaylandContext {
    pub fn init() -> Option<WaylandContext> {
        // attempt to connect to the wayland server
        // this handles both "no libwayland" and "no compositor" cases
        let (display, mut event_queue) = match default_connect() {
            Ok(ret) => ret,
            Err(_) => return None
        };

        // this "expect" cannot trigger (see https://github.com/vberger/wayland-client-rs/issues/69)
        let registry = display.get_registry();
        let env_id = event_queue.add_handler_with_init(WaylandEnv::new(registry));
        // two round trips to fully initialize
        event_queue.sync_roundtrip().expect("Wayland connection unexpectedly lost");
        event_queue.sync_roundtrip().expect("Wayland connection unexpectedly lost");

        {
            let mut state = event_queue.state();
            let mut env = state.get_mut_handler::<WaylandEnv>(env_id);
            if !env.ensure_shell() {
                // This is a compositor bug, it _must_ at least support xl_shell
                panic!("Compositor didi not advertize xdg_shell not wl_shell.");
            }
        }

        Some(WaylandContext {
            evq: Mutex::new(event_queue),
            display: display,
            env_id: env_id
        })
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
        let state = guard.state();
        state.get_handler::<WaylandEnv>(self.env_id).get_seat()
    }

    pub fn with_output<F>(&self, id: MonitorId, f: F) where F: FnOnce(&wl_output::WlOutput) {
        let mut guard = self.evq.lock().unwrap();
        let state = guard.state();
        let env = state.get_handler::<WaylandEnv>(self.env_id);
        for m in env.monitors.iter().filter(|m| m.id == id.id) {
            f(&m.output);
            break
        }
    }

    fn blank_surface(&self, surface: &wl_surface::WlSurface, evq: &mut EventQueue, width: i32, height: i32) {
        let mut tmp = tempfile::tempfile().expect("Failed to create a tmpfile buffer.");
        for _ in 0..(width*height) {
            tmp.write_all(&[0xff,0xff,0xff,0xff]).unwrap();
        }
        tmp.flush().unwrap();
        let pool = {
            let mut state = evq.state();
            let env = state.get_mut_handler::<WaylandEnv>(self.env_id);
            env.inner.shm.create_pool(tmp.as_raw_fd(), width*height*4)
        };
        let buffer = pool.create_buffer(0, width, height, width, wl_shm::Format::Argb8888).expect("Pool cannot be already dead");
        surface.attach(Some(&buffer), 0, 0);
        surface.commit();
        // the buffer will keep the contents alive as needed
        pool.destroy();

        // create a handler to clean up initial buffer
        let init_buffer_handler = InitialBufferHandler {
            initial_buffer: Some((buffer.clone().unwrap(), tmp))
        };
        let initial_buffer_handler_id = evq.add_handler(init_buffer_handler);
        // register the buffer to it
        evq.register::<_, InitialBufferHandler>(&buffer, initial_buffer_handler_id);
    }

    pub fn create_window<H: wayland_window::Handler>(&self, width: u32, height: u32)
        -> (Arc<wl_surface::WlSurface>, wayland_window::DecoratedSurface<H>)
    {
        let mut guard = self.evq.lock().unwrap();
        let (surface, decorated, xdg) = {
            let mut state = guard.state();
            let env = state.get_mut_handler::<WaylandEnv>(self.env_id);
            let surface = Arc::new(env.inner.compositor.create_surface());
            let decorated = wayland_window::DecoratedSurface::new(
                &*surface, 800, 600,
                &env.inner.compositor,
                &env.inner.subcompositor,
                &env.inner.shm,
                env.get_shell(),
                env.get_seat(),
                false
            ).expect("Failed to create a tmpfile buffer.");
            let xdg = match env.get_shell() {
                &Shell::Xdg(_) => true,
                &Shell::Wl(_) => false
            };
            (surface, decorated, xdg)
        };

        if !xdg {
            // if using wl_shell, we need to draw something in order to kickstart
            // the event loop
            // if using xdg_shell, it is an error to do it now, and the events loop will not
            // be stuck. We cannot draw anything before having received an appropriate event
            // from the compositor
            self.blank_surface(&surface, &mut *guard, width as i32, height as i32);
        }
        (surface, decorated)
    }
}

/*
 * Monitors API
 */

pub fn get_primary_monitor(ctxt: &Arc<WaylandContext>) -> MonitorId {
    let mut guard = ctxt.evq.lock().unwrap();
    let state = guard.state();
    let env = state.get_handler::<WaylandEnv>(ctxt.env_id);
    if let Some(ref monitor) = env.monitors.iter().next() {
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
    let env = state.get_handler::<WaylandEnv>(ctxt.env_id);
    env.monitors.iter()
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
        let mut guard = self.ctxt.evq.lock().unwrap();
        let state = guard.state();
        let env = state.get_handler::<WaylandEnv>(self.ctxt.env_id);
        for m in env.monitors.iter().filter(|m| m.id == self.id) {
            return Some(m.name.clone())
        }
        // if we reach here, this monitor does not exist any more
        None
    }

    #[inline]
    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        ::native_monitor::NativeMonitorId::Unavailable
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        let mut guard = self.ctxt.evq.lock().unwrap();
        let state = guard.state();
        let env = state.get_handler::<WaylandEnv>(self.ctxt.env_id);
        for m in env.monitors.iter().filter(|m| m.id == self.id) {
            return m.pix_size
        }
        // if we reach here, this monitor does not exist any more
        (0,0)
    }
}

// a handler to release the ressources acquired to draw the initial white screen as soon as
// the compositor does not use them any more
pub struct InitialBufferHandler {
    initial_buffer: Option<(wl_buffer::WlBuffer, File)>
}

impl wl_buffer::Handler for InitialBufferHandler {
    fn release(&mut self, _: &mut EventQueueHandle, buffer: &wl_buffer::WlBuffer) {
        // release the ressources we've acquired for initial white window
        buffer.destroy();
        self.initial_buffer = None;
    }
}

declare_handler!(InitialBufferHandler, wl_buffer::Handler, wl_buffer::WlBuffer);
