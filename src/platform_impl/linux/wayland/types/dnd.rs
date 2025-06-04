use std::path::PathBuf;

use wayland_client::protocol::wl_surface::WlSurface;

pub struct DndOfferState {
    pub surface: WlSurface,
    pub path: PathBuf,
}
