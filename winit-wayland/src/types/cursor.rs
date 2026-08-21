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
    image: CursorImage,
    representations: Vec<CustomCursorRepresentation>,
}

#[derive(Debug)]
pub struct CustomCursorRepresentation {
    pub buffer: Buffer,
    pub w: i32,
    pub h: i32,
    pub hotspot_x: i32,
    pub hotspot_y: i32,
}

impl CustomCursor {
    pub(crate) fn new(pool: &mut SlotPool, image: &WaylandCustomCursor) -> Self {
        let image = &image.0;
        let representations = image
            .representations()
            .iter()
            .map(|representation| {
                let buffer = image_to_buffer(
                    representation.width() as i32,
                    representation.height() as i32,
                    representation.buffer(),
                    Format::Argb8888,
                    pool,
                )
                .unwrap();

                CustomCursorRepresentation {
                    buffer,
                    w: representation.width() as i32,
                    h: representation.height() as i32,
                    hotspot_x: image.physical_hotspot_x(representation) as i32,
                    hotspot_y: image.physical_hotspot_y(representation) as i32,
                }
            })
            .collect();

        CustomCursor { image: image.clone(), representations }
    }

    pub(crate) fn representation_for_scale_factor(
        &self,
        scale_factor: f64,
    ) -> &CustomCursorRepresentation {
        let representation = self.image.representation_for_scale_factor(scale_factor);
        let index = self
            .image
            .representations()
            .iter()
            .position(|candidate| std::ptr::eq(candidate, representation))
            .expect("cursor image representation should have a matching Wayland buffer");

        &self.representations[index]
    }
}
