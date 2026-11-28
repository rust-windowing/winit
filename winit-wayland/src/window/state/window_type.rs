use dpi::LogicalPosition;
use sctk::shell::WaylandSurface;
use sctk::shell::xdg::dialog::Dialog;
use sctk::shell::xdg::popup::{Popup, PopupConfigure};
use sctk::shell::xdg::window::{Window, WindowConfigure};
use sctk::shell::xdg::{XdgPositioner, XdgSurface};
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
    Dialog {
        dialog: Dialog,
        last_configure: Option<WindowConfigure>,
    },
}

impl WindowType {
    pub fn is_configured(&self) -> bool {
        match self {
            Self::Window { last_configure, .. } => last_configure.is_some(),
            Self::Popup { last_configure, .. } => last_configure.is_some(),
            Self::Dialog { last_configure, .. } => last_configure.is_some(),
        }
    }

    pub fn xdg_toplevel(&self) -> Option<&xdg_toplevel::XdgToplevel> {
        match self {
            WindowType::Window { window, .. } => Some(window.xdg_toplevel()),
            WindowType::Dialog { dialog, .. } => Some(dialog.xdg_toplevel()),
            WindowType::Popup { .. } => None,
        }
    }
}

impl WaylandSurface for WindowType {
    fn wl_surface(&self) -> &wayland_client::protocol::wl_surface::WlSurface {
        match self {
            Self::Window { window, .. } => window.wl_surface(),
            Self::Popup { popup, .. } => popup.wl_surface(),
            Self::Dialog { dialog, .. } => dialog.wl_surface(),
        }
    }
}

impl XdgSurface for WindowType {
    fn xdg_surface(&self) -> &wayland_protocols::xdg::shell::client::xdg_surface::XdgSurface {
        match self {
            Self::Window { window, .. } => window.xdg_surface(),
            Self::Popup { popup, .. } => popup.xdg_surface(),
            Self::Dialog { dialog, .. } => dialog.xdg_surface(),
        }
    }
}
