use sctk::compositor::Region;
use sctk::reexports::client::QueueHandle;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::protocols::ext::background_effect::v1::client::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1;
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur::OrgKdeKwinBlur;

use crate::state::WinitState;
use crate::types::ext_background_effect::ExtBackgroundEffectManager;
use crate::types::kwin_blur::KWinBlurManager;

/// Wrapper around various background effects for [`WlSurface`].
#[derive(Debug, Clone)]
pub enum BgrEffectManager {
    Ext(ExtBackgroundEffectManager),
    KWin(KWinBlurManager),
}

impl BgrEffectManager {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        ExtBackgroundEffectManager::new(globals, queue_handle)
            .map(Self::Ext)
            .or_else(|_| KWinBlurManager::new(globals, queue_handle).map(Self::KWin))
    }

    /// Creates a new blur effect for the surface.
    pub fn new_blur_effect(
        &mut self,
        surface: &WlSurface,
        queue_handle: &QueueHandle<WinitState>,
    ) -> SurfaceBlurEffect {
        match self {
            BgrEffectManager::Ext(mgr) => SurfaceBlurEffect::Ext(mgr.blur(surface, queue_handle)),
            BgrEffectManager::KWin(mgr) => SurfaceBlurEffect::Kwin(
                mgr.blur(surface, queue_handle),
                mgr.clone(),
                surface.clone(),
            ),
        }
    }
}

#[derive(Debug)]
pub enum SurfaceBlurEffect {
    Ext(ExtBackgroundEffectSurfaceV1),
    Kwin(OrgKdeKwinBlur, KWinBlurManager, WlSurface),
}

impl SurfaceBlurEffect {
    /// Returns `true` if the main surface commit is required.
    ///
    /// `None` clears the blur.
    #[must_use]
    pub fn set_blur(&self, region: Option<&Region>) -> bool {
        let region = region.map(|region| region.wl_region());
        match self {
            SurfaceBlurEffect::Ext(surface) => {
                surface.set_blur_region(region);
                true
            },
            SurfaceBlurEffect::Kwin(blur, ..) => {
                blur.set_region(region);
                blur.commit();
                true
            },
        }
    }
}

impl Drop for SurfaceBlurEffect {
    fn drop(&mut self) {
        match self {
            SurfaceBlurEffect::Ext(surface) => surface.destroy(),
            SurfaceBlurEffect::Kwin(blur, mgr, wl_surface) => {
                blur.set_region(None);
                blur.commit();
                blur.release();
                mgr.unset(wl_surface);
            },
        }
    }
}
