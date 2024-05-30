//! Fill the window buffer with a solid color.
//!
//! Launching a window without drawing to it has unpredictable results varying from platform to
//! platform. In order to have well-defined examples, this module provides an easy way to
//! fill the window buffer with a solid color.
//!
//! The `softbuffer` crate is used, largely because of its ease of use. `glutin` or `wgpu` could
//! also be used to fill the window buffer, but they are more complicated to use.

#![allow(unused)]
pub use platform::{cleanup_window, fill_window, fill_window_with_border};

#[cfg(all(feature = "rwh_05", not(any(target_os = "android", target_os = "ios"))))]
mod platform {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::mem;
    use std::mem::ManuallyDrop;
    use std::num::NonZeroU32;

    use softbuffer::{Buffer, Context, Surface};
    use winit::window::{Window, WindowId};

    thread_local! {
        // NOTE: You should never do things like that, create context and drop it before
        // you drop the event loop. We do this for brevity to not blow up examples. We use
        // ManuallyDrop to prevent destructors from running.
        //
        // A static, thread-local map of graphics contexts to open windows.
        static GC: ManuallyDrop<RefCell<Option<GraphicsContext>>> = const { ManuallyDrop::new(RefCell::new(None)) };
    }

    /// The graphics context used to draw to a window.
    struct GraphicsContext {
        /// The global softbuffer context.
        context: RefCell<Context<&'static Window>>,

        /// The hash map of window IDs to surfaces.
        surfaces: HashMap<WindowId, Surface<&'static Window, &'static Window>>,
    }

    impl GraphicsContext {
        fn new(w: &Window) -> Self {
            Self {
                context: RefCell::new(
                    Context::new(unsafe { mem::transmute::<&'_ Window, &'static Window>(w) })
                        .expect("Failed to create a softbuffer context"),
                ),
                surfaces: HashMap::new(),
            }
        }

        fn create_surface(
            &mut self,
            window: &Window,
        ) -> &mut Surface<&'static Window, &'static Window> {
            self.surfaces.entry(window.id()).or_insert_with(|| {
                Surface::new(&self.context.borrow(), unsafe {
                    mem::transmute::<&'_ Window, &'static Window>(window)
                })
                .expect("Failed to create a softbuffer surface")
            })
        }

        fn destroy_surface(&mut self, window: &Window) {
            self.surfaces.remove(&window.id());
        }
    }

    const DARK_GRAY: u32 = 0xff181818;
    const LEMON: u32 = 0xffd1ffbd;

    pub fn fill_window(window: &Window) {
        fill_window_ex(window, |_, _, buffer| buffer.fill(DARK_GRAY))
    }

    pub fn fill_window_with_border(window: &Window) {
        fill_window_ex(window, |width, height, buffer| {
            for y in 0..height {
                for x in 0..width {
                    let color = if (x == 0 || y == 0 || x == width - 1 || y == height - 1) {
                        LEMON
                    } else {
                        DARK_GRAY
                    };
                    buffer[y * width + x] = color;
                }
            }
        })
    }
    pub fn fill_window_ex<F: Fn(usize, usize, &mut Buffer<&Window, &Window>)>(
        window: &Window,
        f: F,
    ) {
        GC.with(|gc| {
            let size = window.inner_size();
            let (Some(width), Some(height)) =
                (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
            else {
                return;
            };

            // Either get the last context used or create a new one.
            let mut gc = gc.borrow_mut();
            let surface =
                gc.get_or_insert_with(|| GraphicsContext::new(window)).create_surface(window);

            surface.resize(width, height).expect("Failed to resize the softbuffer surface");

            let mut buffer = surface.buffer_mut().expect("Failed to get the softbuffer buffer");

            let width = width.get() as usize;
            let height = height.get() as usize;

            f(width, height, &mut buffer);

            buffer.present().expect("Failed to present the softbuffer buffer");
        })
    }

    #[allow(dead_code)]
    pub fn cleanup_window(window: &Window) {
        GC.with(|gc| {
            let mut gc = gc.borrow_mut();
            if let Some(context) = gc.as_mut() {
                context.destroy_surface(window);
            }
        });
    }
}

#[cfg(not(all(feature = "rwh_05", not(any(target_os = "android", target_os = "ios")))))]
mod platform {
    pub fn fill_window(_window: &winit::window::Window) {
        // No-op on mobile platforms.
    }

    pub fn fill_window_with_border(window: &Window) {
        // No-op on mobile platforms.
    }

    #[allow(dead_code)]
    pub fn cleanup_window(_window: &winit::window::Window) {
        // No-op on mobile platforms.
    }
}
