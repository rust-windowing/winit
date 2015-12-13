use Event as GlutinEvent;

use wayland_client::Event as WaylandEvent;
use wayland_client::ProxyId;

pub fn translate_event(evt: WaylandEvent) -> Option<(GlutinEvent, ProxyId)> {
    match evt {
        _ => None
    }
}