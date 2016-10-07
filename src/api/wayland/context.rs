use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use wayland_client::{EnvHandler, default_connect, EventQueue, EventQueueHandle, Init, Proxy};
use wayland_client::protocol::{wl_compositor, wl_seat, wl_shell, wl_shm, wl_subcompositor,
                               wl_display, wl_registry, wl_output};

/*
 * Registry and globals handling
 */

wayland_env!(InnerEnv,
    compositor: wl_compositor::WlCompositor,
    seat: wl_seat::WlSeat,
    shell: wl_shell::WlShell,
    shm: wl_shm::WlShm,
    subcompositor: wl_subcompositor::WlSubcompositor
);

struct WaylandEnv {
    registry: wl_registry::WlRegistry,
    inner: EnvHandler<InnerEnv>,
    monitors: Vec<OutputInfo>,
    my_id: usize
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
            monitors: Vec::new(),
            my_id: 0
        }
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
        if interface == "wl_output" {
            // intercept outputs
            // this "expect" cannot trigger (see https://github.com/vberger/wayland-client-rs/issues/69)
            let output = self.registry.bind::<wl_output::WlOutput>(1, name)
                             .expect("Registry cannot be dead");
            evqh.register::<_, WaylandEnv>(&output, self.my_id);
            self.monitors.push(OutputInfo::new(output, name));
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
            Err(e) => return None
        };

        // this expect cannot trigger (see https://github.com/vberger/wayland-client-rs/issues/69)
        let registry = display.get_registry().expect("Display cannot be already destroyed.");
        let env_id = event_queue.add_handler_with_init(WaylandEnv::new(registry));
        event_queue.sync_roundtrip().expect("Wayland connection unexpectedly lost");

        Some(WaylandContext {
            evq: Mutex::new(event_queue),
            display: display,
            env_id: env_id
        })
    }
}

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
