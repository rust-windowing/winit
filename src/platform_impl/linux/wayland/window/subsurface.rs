use std::sync::{Arc, RwLock};

use sctk::{
    compositor::SurfaceData,
    shell::WaylandSurface,
    subcompositor::{SubcompositorState, SubsurfaceData},
};
use wayland_client::{
    protocol::{wl_subsurface::WlSubsurface, wl_surface::WlSurface},
    Dispatch, QueueHandle,
};

use crate::dpi::PhysicalPosition;

#[derive(Clone)]
pub struct Subsurface(Arc<SubsurfaceInner>);

struct SubsurfaceInner {
    pub surface: WlSurface,
    pub subsurface: WlSubsurface,
    pub position: RwLock<PhysicalPosition<i32>>,
}

impl Subsurface {
    pub fn from_parent<T>(
        parent: &WlSurface,
        subcompositor: &SubcompositorState,
        queue_handle: &QueueHandle<T>,
    ) -> Subsurface
    where
        T: Dispatch<WlSurface, SurfaceData> + Dispatch<WlSubsurface, SubsurfaceData> + 'static,
    {
        let (subsurface, surface) = subcompositor.create_subsurface(parent.clone(), queue_handle);

        Subsurface(Arc::new(SubsurfaceInner {
            surface,
            subsurface,
            position: RwLock::new(PhysicalPosition::new(0, 0)),
        }))
    }

    pub fn set_position(&self, pos: PhysicalPosition<i32>) {
        let inner = &*self.0;

        inner.subsurface.set_position(pos.x, pos.y);
        *inner.position.write().unwrap() = pos;
    }

    pub fn get_position(&self) -> PhysicalPosition<i32> {
        *self.0.position.read().unwrap()
    }

    pub fn set_sync(&self) {
        self.0.subsurface.set_sync();
    }

    pub fn set_desync(&self) {
        self.0.subsurface.set_desync();
    }
}

impl WaylandSurface for Subsurface {
    fn wl_surface(&self) -> &wayland_client::protocol::wl_surface::WlSurface {
        &self.0.surface
    }
}
