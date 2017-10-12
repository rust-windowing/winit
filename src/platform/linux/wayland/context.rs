use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use wayland_client::{EnvHandler, EnvNotify, default_connect, EventQueue, EventQueueHandle, Proxy, StateToken};
use wayland_client::protocol::{wl_compositor, wl_seat, wl_shell, wl_shm, wl_subcompositor,
                               wl_display, wl_registry, wl_output, wl_surface, wl_buffer};

use super::wayland_protocols::unstable::xdg_shell::client::zxdg_shell_v6;

use super::wayland_window::Shell;

pub struct WaylandContext {
    display: wl_display::WlDisplay,
    evqh: Mutex<EventQueue>,
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
            evqh: Mutex::new(event_queue),
            env_token: env_token,
            ctxt_token: ctxt_token
        })
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
                // evqh.register::<_, WaylandEnv>(&output, self.my_id);
                evqh.state().get_mut(&token).monitors.push(OutputInfo::new(output, id));
            } else if interface == zxdg_shell_v6::ZxdgShellV6::interface_name() {
                // We have an xdg_shell, bind it
                let xdg_shell = registry.bind::<zxdg_shell_v6::ZxdgShellV6>(1, id);
                //let xdg_ping_hid = evqh.add_handler(XdgShellPingHandler);
                //evqh.register::<_, XdgShellPingHandler>(&xdg_shell, xdg_ping_hid);
                evqh.state().get_mut(&token).shell = Some(Shell::Xdg(xdg_shell));
            } else if interface == wl_seat::WlSeat::interface_name() {
                // FIXME: currently we only take first seat, what to do when
                // multiple seats ?
                if evqh.state().get_mut(&token).seat.is_none() {
                    if version < 5 {
                        panic!("Winit requires at least version 5 of the wl_seat global.");
                    }
                    let seat = registry.bind::<wl_seat::WlSeat>(5, id);
                    // TODO: register
                    evqh.state().get_mut(&token).seat = Some(seat);
                }
            }
        },
        del_global: |evqh, token, _, id| {
            // maybe this was a monitor, cleanup
            evqh.state().get_mut(&token).monitors.retain(|m| m.id != id);
        },
        ready: |_, _, _| {}
    }
}

/*
 * Monitor stuff
 */

struct OutputInfo {
    output: wl_output::WlOutput,
    id: u32,
    scale: f32,
    pix_size: (u32, u32),
    pix_pos: (u32, u32),
    name: String
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
    unimplemented!()
}

pub fn get_available_monitors(ctxt: &Arc<WaylandContext>) -> VecDeque<MonitorId> {
    unimplemented!()
}

#[derive(Clone)]
pub struct MonitorId;

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        unimplemented!()
    }

    #[inline]
    pub fn get_native_identifier(&self) -> u32 {
        unimplemented!()
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!()
    }

    pub fn get_position(&self) -> (i32, i32) {
            unimplemented!()
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f32 {
        unimplemented!()
    }
}
