use sctk::shell::{xdg::window::Window, WaylandSurface};

use crate::platform::wayland::Theme;

use super::{into_sctk_adwaita_config, WinitFrame};

pub(in crate::platform_impl::platform::wayland) struct ToplevelState {
    /// The underlying SCTK window.
    pub window: Window,

    /// The window frame, which is created from the configure request.
    pub frame: Option<WinitFrame>,

    /// Whether the client side decorations have pending move operations.
    ///
    /// The value is the serial of the event triggered moved.
    pub has_pending_move: Option<u32>,

    /// The current window title.
    pub title: String,

    /// Whether the frame is resizable.
    pub resizable: bool,

    /// Whether the CSD fail to create, so we don't try to create them on each iteration.
    pub csd_fails: bool,

    /// Whether we should decorate the frame.
    pub decorate: bool,
}

impl ToplevelState {
    pub fn set_theme(&mut self, theme: Option<Theme>) {
        #[cfg(feature = "sctk-adwaita")]
        if let Some(frame) = self.frame.as_mut() {
            frame.set_config(into_sctk_adwaita_config(theme))
        }
    }
}

impl WaylandSurface for ToplevelState {
    fn wl_surface(&self) -> &wayland_client::protocol::wl_surface::WlSurface {
        self.window.wl_surface()
    }
}