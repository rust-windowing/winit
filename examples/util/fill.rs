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
    use softbuffer::GraphicsContext;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use winit::window::WindowId;

    thread_local! {
        /// A static, thread-local map of graphics contexts to open windows.
        static GC: RefCell<HashMap<WindowId, GraphicsContext>> = RefCell::new(HashMap::new());
    }

    GC.with(|gc| {
        // Either get the last context used or create a new one.
        let mut gc = gc.borrow_mut();
        let context = gc.entry(window.id()).or_insert_with(|| unsafe {
            GraphicsContext::new(window, window)
                .expect("Failed to create a softbuffer graphics context")
        });

        // Fill a buffer with a solid color.
        const LIGHT_GRAY: u32 = 0xFFD3D3D3;
        let size = window.inner_size();
        let buffer = vec![LIGHT_GRAY; size.width as usize * size.height as usize];

        // Draw the buffer to the window.
        context.set_buffer(&buffer, size.width as u16, size.height as u16);
    })
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub(super) fn fill_window(_window: &Window) {
    // No-op on mobile platforms.
}
