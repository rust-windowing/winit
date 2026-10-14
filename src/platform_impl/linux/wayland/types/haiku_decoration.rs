//! Native Haiku window borders through the legacy KDE decoration protocol.

use crate::platform_impl::wayland::state::WinitState;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{delegate_noop, QueueHandle};
use wayland_protocols_plasma::server_decoration::client::org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration;
use wayland_protocols_plasma::server_decoration::client::org_kde_kwin_server_decoration_manager::OrgKdeKwinServerDecorationManager;

pub struct HaikuDecorationManager(OrgKdeKwinServerDecorationManager);

impl HaikuDecorationManager {
    pub fn bind(globals: &GlobalList, queue: &QueueHandle<WinitState>) -> Result<Self, BindError> {
        globals.bind(queue, 1..=1, ()).map(Self)
    }

    pub fn create(
        &self,
        surface: &WlSurface,
        queue: &QueueHandle<WinitState>,
    ) -> OrgKdeKwinServerDecoration {
        let decoration = self.0.create(surface, queue, ());
        decoration.request_mode(2);
        decoration
    }
}

delegate_noop!(WinitState: ignore OrgKdeKwinServerDecorationManager);
delegate_noop!(WinitState: ignore OrgKdeKwinServerDecoration);
