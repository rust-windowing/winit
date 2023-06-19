//! Fill the window buffer with a solid color.
//!
//! Launching a window without drawing to it has unpredictable results varying from platform to
//! platform. In order to have well-defined examples, this module provides an easy way to
//! fill the window buffer with a solid color.
//!
//! The `softbuffer` crate is used, largely because of its ease of use. `glutin` or `wgpu` could
//! also be used to fill the window buffer, but they are more complicated to use.

use winit::window::Window;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) fn fill_window(window: &Window) {
    use softbuffer::{Context, Surface};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::num::NonZeroU32;
    use winit::window::WindowId;

    /// The graphics context used to draw to a window.
    struct GraphicsContext {
        /// The global softbuffer context.
        context: Context,

        /// The hash map of window IDs to surfaces.
        surfaces: HashMap<WindowId, Surface>,
    }

    impl GraphicsContext {
        fn new(w: &Window) -> Self {
            Self {
                context: unsafe { Context::new(w) }.expect("Failed to create a softbuffer context"),
                surfaces: HashMap::new(),
            }
        }

        fn surface(&mut self, w: &Window) -> &mut Surface {
            self.surfaces.entry(w.id()).or_insert_with(|| {
                unsafe { Surface::new(&self.context, w) }
                    .expect("Failed to create a softbuffer surface")
            })
        }
    }

    thread_local! {
        /// A static, thread-local map of graphics contexts to open windows.
        static GC: RefCell<Option<GraphicsContext>> = RefCell::new(None);
    }

    GC.with(|gc| {
        // Either get the last context used or create a new one.
        let mut gc = gc.borrow_mut();
        let surface = gc
            .get_or_insert_with(|| GraphicsContext::new(window))
            .surface(window);

        // Fill a buffer with a solid color.
        const DARK_GRAY: u32 = 0xFF181818;
        let size = window.inner_size();

        surface
            .resize(
                NonZeroU32::new(size.width).expect("Width must be greater than zero"),
                NonZeroU32::new(size.height).expect("Height must be greater than zero"),
            )
            .expect("Failed to resize the softbuffer surface");

        let mut buffer = surface
            .buffer_mut()
            .expect("Failed to get the softbuffer buffer");
        buffer.fill(DARK_GRAY);
        buffer
            .present()
            .expect("Failed to present the softbuffer buffer");
    })
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub(super) fn fill_window(_window: &Window) {
    // No-op on mobile platforms.
}
