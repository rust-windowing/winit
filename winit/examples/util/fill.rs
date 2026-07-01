use std::num::NonZeroU32;

use rwh_06::{HasDisplayHandle, HasWindowHandle};
use softbuffer::Surface;
use winit::window::Window;

/// Resize the surface.
pub fn resize(
    surface: &mut Surface<impl HasDisplayHandle, impl HasWindowHandle>,
    surface_size: dpi::PhysicalSize<u32>,
) {
    // Handle zero-sized buffers.
    //
    // FIXME(madsmtm): This should be done by softbuffer internally in the future:
    // https://github.com/rust-windowing/softbuffer/issues/238
    let (Some(width), Some(height)) =
        (NonZeroU32::new(surface_size.width), NonZeroU32::new(surface_size.height))
    else {
        return;
    };

    surface.resize(width, height).expect("Failed to resize the softbuffer surface");
}

/// Fill the window buffer with a solid color.
pub fn fill_with_color(
    surface: &mut Surface<impl HasDisplayHandle, impl HasWindowHandle + AsRef<dyn Window>>,
    color: u32,
) {
    let surface_size = surface.window().as_ref().surface_size();
    resize(surface, surface_size);

    let mut buffer = surface.buffer_mut().expect("Failed to get the softbuffer buffer");
    buffer.fill(color);
    buffer.present().expect("Failed to present the softbuffer buffer");
}

#[allow(dead_code)]
pub fn fill(
    surface: &mut Surface<impl HasDisplayHandle, impl HasWindowHandle + AsRef<dyn Window>>,
) {
    fill_with_color(surface, 0xff181818);
}
