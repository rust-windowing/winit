use cursor_icon::CursorIcon;

use sctk::reexports::client::protocol::wl_shm::Format;
use sctk::shm::slot::{Buffer, SlotPool};

use crate::cursor::CursorImage;

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
    pub fn new(pool: &mut SlotPool, image: &CursorImage) -> Self {
        let (buffer, canvas) = pool
            .create_buffer(
                image.width as i32,
                image.height as i32,
                4 * (image.width as i32),
                Format::Argb8888,
            )
            .unwrap();

        for (canvas_chunk, rgba_chunk) in canvas.chunks_exact_mut(4).zip(image.rgba.chunks_exact(4))
        {
            canvas_chunk[0] = rgba_chunk[2];
            canvas_chunk[1] = rgba_chunk[1];
            canvas_chunk[2] = rgba_chunk[0];
            canvas_chunk[3] = rgba_chunk[3];
        }

        CustomCursor {
            buffer,
            w: image.width as i32,
            h: image.height as i32,
            hotspot_x: image.hotspot_x as i32,
            hotspot_y: image.hotspot_y as i32,
        }
    }
}
