use sctk::shell::WaylandSurface;
use wayland_client::protocol::{wl_subsurface::WlSubsurface, wl_surface::WlSurface};

#[allow(unused)]
pub(in crate::platform_impl::platform::wayland) struct SubsurfaceState {
    /// The parent surface for this subsurface.
    pub parent_surface: WlSurface,

    /// The surface.
    pub surface: WlSurface,

    /// The subsurface role. Allows control over position.
    pub subsurface: WlSubsurface
}

impl WaylandSurface for SubsurfaceState {
    fn wl_surface(&self) -> &wayland_client::protocol::wl_surface::WlSurface {
        return &self.surface;
    }
}