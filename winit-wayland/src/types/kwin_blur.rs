//! Handling of KDE-compatible blur.

use sctk::globals::GlobalData;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur::OrgKdeKwinBlur;
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager;

use crate::state::WinitState;

/// KWin blur manager.
#[derive(Debug, Clone)]
pub struct KWinBlurManager {
    manager: OrgKdeKwinBlurManager,
}

impl KWinBlurManager {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let manager = globals.bind_singleton(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { manager })
    }

    pub fn blur(
        &self,
        surface: &WlSurface,
        queue_handle: &QueueHandle<WinitState>,
    ) -> OrgKdeKwinBlur {
        self.manager.create(surface, queue_handle, ())
    }

    pub fn unset(&self, surface: &WlSurface) {
        self.manager.unset(surface)
    }
}

impl Dispatch<OrgKdeKwinBlurManager, WinitState> for GlobalData {
    fn event(
        &self,
        _: &mut WinitState,
        _: &OrgKdeKwinBlurManager,
        _: <OrgKdeKwinBlurManager as Proxy>::Event,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        unreachable!("no events defined for org_kde_kwin_blur_manager");
    }
}

impl Dispatch<OrgKdeKwinBlur, WinitState> for () {
    fn event(
        &self,
        _: &mut WinitState,
        _: &OrgKdeKwinBlur,
        _: <OrgKdeKwinBlur as Proxy>::Event,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        unreachable!("no events defined for org_kde_kwin_blur");
    }
}
