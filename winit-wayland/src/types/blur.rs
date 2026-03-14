use std::sync::Arc;

use dpi::LogicalSize;
use sctk::compositor::CompositorState;
use sctk::reexports::client::QueueHandle;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::protocols::ext::background_effect::v1::client::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1;
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur::OrgKdeKwinBlur;

use crate::state::WinitState;
use crate::types::ext_background_effect::BackgroundEffectManager;
use crate::types::kwin_blur::KWinBlurManager;

#[derive(Debug)]
pub enum BlurSurface {
    Ext(WlSurface, ExtBackgroundEffectSurfaceV1),
    Kwin(OrgKdeKwinBlur),
}

impl BlurSurface {
    pub fn commit(&self) {
        match self {
            BlurSurface::Ext(s, _) => s.commit(),
            BlurSurface::Kwin(s) => s.commit(),
        }
    }
}

impl Drop for BlurSurface {
    fn drop(&mut self) {
        match self {
            BlurSurface::Ext(_, s) => s.destroy(),
            BlurSurface::Kwin(s) => s.release(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum BlurManager {
    Ext(BackgroundEffectManager),
    KWin(KWinBlurManager),
}

impl BlurManager {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        match BackgroundEffectManager::new(globals, queue_handle) {
            Ok(m) => Ok(Self::Ext(m)),
            Err(e) => {
                if let Ok(m) = KWinBlurManager::new(globals, queue_handle) {
                    Ok(Self::KWin(m))
                } else {
                    Err(e)
                }
            },
        }
    }

    pub fn blur(
        &mut self,
        compositor_state: &Arc<CompositorState>,
        surface: &WlSurface,
        queue_handle: &QueueHandle<WinitState>,
        size: LogicalSize<u32>,
    ) -> BlurSurface {
        match self {
            BlurManager::Ext(m) => BlurSurface::Ext(
                surface.clone(),
                m.blur(compositor_state, surface, queue_handle, size),
            ),
            BlurManager::KWin(m) => BlurSurface::Kwin(m.blur(surface, queue_handle)),
        }
    }

    pub fn unset(&mut self, surface: &WlSurface) {
        match self {
            BlurManager::Ext(m) => m.unset(surface),
            BlurManager::KWin(m) => m.unset(surface),
        }
    }
}
