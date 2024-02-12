use wayland_client::protocol::{wl_subsurface::WlSubsurface, wl_surface::WlSurface};

struct SubsurfaceState {
    parent_surface: WlSurface,
    surface: WlSurface,
    subsurface: WlSubsurface
}