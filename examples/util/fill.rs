//! Fill the window buffer with a solid color.
//!
//! Launching a window without drawing to it has unpredictable results varying from platform to
//! platform. In order to have well-defined examples, this module provides an easy way to
//! fill the window buffer with a solid color.
//!
//! The `softbuffer` crate is used, largely because of its ease of use. `glutin` or `wgpu` could
//! also be used to fill the window buffer, but they are more complicated to use.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

// Abstract over Rc<Window> and Arc<Window>
pub(super) trait FullHandleTy:
    AsRef<Window> + HasDisplayHandle + HasWindowHandle + 'static
{
}
impl<T: AsRef<Window> + HasDisplayHandle + HasWindowHandle + 'static> FullHandleTy for T {}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod fill {
    use super::FullHandleTy;
    use softbuffer::{Context, Surface};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::num::NonZeroU32;
    use std::rc::Rc;
    use winit::window::WindowId;

    type FullHandle = Rc<dyn FullHandleTy>;

    /// The graphics context used to draw to a window.
    struct GraphicsContext {
        /// The global softbuffer context.
        context: Context<FullHandle>,

        /// The hash map of window IDs to surfaces.
        surfaces: HashMap<WindowId, Surface<FullHandle, FullHandle>>,
    }

    impl GraphicsContext {
        fn new(w: &(impl FullHandleTy + Clone)) -> Self {
            let x: FullHandle = Rc::new(w.clone());
            Self {
                context: Context::new(x).expect("Failed to create a softbuffer context"),
                surfaces: HashMap::new(),
            }
        }

        fn surface(
            &mut self,
            w: &(impl FullHandleTy + Clone),
        ) -> &mut Surface<FullHandle, FullHandle> {
            self.surfaces.entry(w.as_ref().id()).or_insert_with(|| {
                let x: FullHandle = Rc::new(w.clone());
                Surface::new(&self.context, x).expect("Failed to create a softbuffer surface")
            })
        }
    }

    thread_local! {
        // A static, thread-local map of graphics contexts to open windows.
        static GC: RefCell<Option<GraphicsContext>> = RefCell::new(None);
    }

    pub(crate) fn fill_window(window: &(impl FullHandleTy + Clone)) {
        GC.with(|gc| {
            // Either get the last context used or create a new one.
            let mut gc = gc.borrow_mut();
            let surface = gc
                .get_or_insert_with(|| GraphicsContext::new(window))
                .surface(window);

            // Fill a buffer with a solid color.
            const DARK_GRAY: u32 = 0xFF181818;
            let size = window.as_ref().inner_size();

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

    pub(crate) fn discard_window(window: &WindowId) {
        GC.with(|gc| {
            let mut gc = gc.borrow_mut();
            if let Some(gc) = &mut *gc {
                gc.surfaces.remove(window);
            }
        })
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
mod fill {
    use super::FullHandleTy;
    use winit::window::WindowId;

    pub(crate) fn fill_window(_window: &impl FullHandleTy) {
        // No-op on mobile platforms.
    }

    pub(crate) fn discard_window(_window: &WindowId) {
        // No-op on mobile platforms.
    }
}

pub(super) use fill::{discard_window, fill_window};
