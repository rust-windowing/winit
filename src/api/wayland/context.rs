use std::collections::VecDeque;

use wayland_client::protocol::{wl_compositor, wl_seat, wl_shell, wl_shm, wl_subcompositor};

wayland_env!(InnerEnv,
    compositor: wl_compositor::WlCompositor,
    seat: wl_seat::WlSeat,
    shell: wl_shell::WlShell,
    shm: wl_shm::WlShm,
    subcompositor: wl_subcompositor::WlSubcompositor
);

pub struct WaylandContext {
}

impl WaylandContext {
    pub fn init() -> Option<WaylandContext> {
        None
    }
    
    pub fn get_primary_monitor(&self) -> MonitorId {
        unimplemented!()
    }
    
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        unimplemented!()
    }
}

#[derive(Clone)]
pub struct MonitorId;

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        unimplemented!()
    }

    #[inline]
    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        ::native_monitor::NativeMonitorId::Unavailable
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!()
    }
}
