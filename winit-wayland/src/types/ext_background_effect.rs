use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use dpi::LogicalSize;
use sctk::compositor::{CompositorState, Region};
use sctk::globals::GlobalData;
use sctk::reexports::client::backend::ObjectId;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{Connection, Dispatch, Proxy, QueueHandle, delegate_dispatch};
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1;
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1;

use crate::state::WinitState;

#[derive(Debug, Clone)]
pub struct BackgroundEffectManager {
    manager: ExtBackgroundEffectManagerV1,
    surfaces: HashMap<ObjectId, ExtBackgroundEffectSurfaceV1>,
}

impl BackgroundEffectManager {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { manager, surfaces: HashMap::new() })
    }

    pub fn blur(
        &mut self,
        compositor_state: &Arc<CompositorState>,
        surface: &WlSurface,
        queue_handle: &QueueHandle<WinitState>,
        size: LogicalSize<u32>,
    ) -> ExtBackgroundEffectSurfaceV1 {
        let region = Region::new(compositor_state.deref()).unwrap();
        region.add(0, 0, size.width as i32, size.height as i32);
        let surface = if let Some(existing) = self.surfaces.get(&surface.id()) {
            existing.clone()
        } else {
            let surface = self.manager.get_background_effect(surface, queue_handle, ());
            self.surfaces.insert(surface.id(), surface.clone());
            surface
        };
        surface.set_blur_region(Some(region.wl_region()));
        surface
    }

    pub fn unset(&mut self, surface: &WlSurface) {
        self.surfaces.remove(&surface.id());
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, GlobalData, WinitState> for BackgroundEffectManager {
    fn event(
        _: &mut WinitState,
        _: &ExtBackgroundEffectManagerV1,
        _: <ExtBackgroundEffectManagerV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<ExtBackgroundEffectSurfaceV1, (), WinitState> for BackgroundEffectManager {
    fn event(
        _: &mut WinitState,
        _: &ExtBackgroundEffectSurfaceV1,
        _: <ExtBackgroundEffectSurfaceV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        // There is no event
    }
}

delegate_dispatch!(WinitState: [ExtBackgroundEffectManagerV1: GlobalData] => BackgroundEffectManager);
delegate_dispatch!(WinitState: [ExtBackgroundEffectSurfaceV1: ()] => BackgroundEffectManager);
