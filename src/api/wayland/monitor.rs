use std::collections::VecDeque;

use wayland_client::{ProxyId, EventIterator};
use wayland_client::wayland::output::WlOutput;

use super::context::WAYLAND_CONTEXT;

#[derive(Clone)]
pub struct MonitorId(ProxyId);

#[inline]
pub fn get_available_monitors() -> VecDeque<MonitorId> {
    WAYLAND_CONTEXT.as_ref().map(|ctxt|
        ctxt.monitor_ids().into_iter().map(MonitorId).collect()
    ).unwrap_or(VecDeque::new())
}
#[inline]
pub fn get_primary_monitor() -> MonitorId {
    WAYLAND_CONTEXT.as_ref().and_then(|ctxt|
        ctxt.monitor_ids().into_iter().next().map(MonitorId)
    ).expect("wayland: No monitor available.")
}

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        WAYLAND_CONTEXT.as_ref().and_then(|ctxt| ctxt.monitor_name(self.0))
    }

    #[inline]
    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        ::native_monitor::NativeMonitorId::Unavailable
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        WAYLAND_CONTEXT.as_ref().and_then(|ctxt| ctxt.monitor_dimensions(self.0)).unwrap()
    }
}

pub fn proxid_from_monitorid(x: &MonitorId) -> ProxyId {
    x.0
}

pub fn init_monitors(outputs: &mut Vec<(WlOutput, u32, u32, String)>, evts: EventIterator) {
    use wayland_client::{Event, Proxy};
    use wayland_client::wayland::WaylandProtocolEvent;
    use wayland_client::wayland::output::{WlOutputEvent, WlOutputMode};

    for evt in evts {
        match evt {
            Event::Wayland(WaylandProtocolEvent::WlOutput(pid, oevt)) => match oevt {
                WlOutputEvent::Geometry(_, _, _, _, _, maker, model, _) => {
                    for o in outputs.iter_mut() {
                        if o.0.id() == pid {
                            o.3 = format!("{} - {}", maker, model);
                            break
                        }
                    }
                },
                WlOutputEvent::Mode(flags, width, height, _) => {
                    if flags.contains(WlOutputMode::Current) {
                        for o in outputs.iter_mut() {
                            if o.0.id() == pid {
                                o.1 = width as u32;
                                o.2 = height as u32;
                                break
                            }
                        }
                    }
                },
                _ => {}
            },
            _ => {}
        }
    }
}