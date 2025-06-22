use cursor_icon::CursorIcon;
use sctk::reexports::client::protocol::wl_shm::Format;
use sctk::shm::slot::{Buffer, SlotPool};
use winit_core::cursor::{CursorImage, CustomCursorProvider};

use crate::image_to_buffer;

// Wrap in our own type to not impl trait on global type.
#[derive(Debug)]
pub struct WaylandCustomCursor(pub(crate) CursorImage);
impl CustomCursorProvider for WaylandCustomCursor {
    fn is_animated(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum SelectedCursor {
    Named(CursorIcon),
    Custom(CustomCursor),
}

impl Default for SelectedCursor {
    fn default() -> Self {
        Self::Named(Default::default())
    }
}

#[derive(Debug)]
pub struct CustomCursor {
    pub buffer: Buffer,
    pub w: i32,
    pub h: i32,
    pub hotspot_x: i32,
    pub hotspot_y: i32,
}

impl CustomCursor {
    pub(crate) fn new(pool: &mut SlotPool, image: &WaylandCustomCursor) -> Self {
        let image = &image.0;
        let buffer = image_to_buffer(
            image.width() as i32,
            image.height() as i32,
            image.buffer(),
            Format::Argb8888,
            pool,
        )
        .unwrap();

        CustomCursor {
            buffer,
            w: image.width() as i32,
            h: image.height() as i32,
            hotspot_x: image.hotspot_x() as i32,
            hotspot_y: image.hotspot_y() as i32,
        }
    }
}
