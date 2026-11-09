use dpi::LogicalPosition;
use sctk::shell::WaylandSurface;
use sctk::shell::wlr_layer::{LayerSurface, LayerSurfaceConfigure};
use sctk::shell::xdg::popup::{Popup, PopupConfigure};
use sctk::shell::xdg::window::{Window, WindowConfigure};
use sctk::shell::xdg::XdgPositioner;
use wayland_protocols::xdg::shell::client::xdg_toplevel;
use winit_core::window::WindowPositioner;

#[derive(Debug)]
pub enum WindowType {
    // The option is the last received configure
    Window {
        window: Window,
        last_configure: Option<WindowConfigure>,
    },
    Popup {
        popup: Popup,
        xdg_positioner: XdgPositioner,
        last_configure: Option<PopupConfigure>,
        parent_origin: LogicalPosition<i32>,

        positioner: WindowPositioner,
    },
    Layer {
        surface: LayerSurface,
        last_configure: Option<LayerSurfaceConfigure>,
    },
}

impl WindowType {
    pub fn is_configured(&self) -> bool {
        match self {
            Self::Window { last_configure, .. } => last_configure.is_some(),
            Self::Popup { last_configure, .. } => last_configure.is_some(),
            Self::Layer { last_configure, .. } => last_configure.is_some(),
        }
    }

    pub fn xdg_toplevel(&self) -> Option<&xdg_toplevel::XdgToplevel> {
        match self {
            WindowType::Window { window, .. } => Some(window.xdg_toplevel()),
            WindowType::Popup { .. } | WindowType::Layer { .. } => None,
        }
    }

    pub fn as_layer(&self) -> Option<&LayerSurface> {
        match self {
            WindowType::Layer { surface, .. } => Some(surface),
            _ => None,
        }
    }

    pub fn xdg_surface(
        &self,
    ) -> Option<&wayland_protocols::xdg::shell::client::xdg_surface::XdgSurface> {
        use sctk::shell::xdg::XdgSurface as _;
        match self {
            Self::Window { window, .. } => Some(window.xdg_surface()),
            Self::Popup { popup, .. } => Some(popup.xdg_surface()),
            Self::Layer { .. } => None,
        }
    }
}

impl WaylandSurface for WindowType {
    fn wl_surface(&self) -> &wayland_client::protocol::wl_surface::WlSurface {
        match self {
            Self::Window { window, .. } => window.wl_surface(),
            Self::Popup { popup, .. } => popup.wl_surface(),
            Self::Layer { surface, .. } => surface.wl_surface(),
        }
    }
}

