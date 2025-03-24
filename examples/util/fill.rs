//! Fill the window buffer with a solid color.
//!
//! Launching a window without drawing to it has unpredictable results varying from platform to
//! platform. In order to have well-defined examples, this module provides an easy way to
//! fill the window buffer with a solid color.
//!
//! The `softbuffer` crate is used, largely because of its ease of use. `glutin` or `wgpu` could
//! also be used to fill the window buffer, but they are more complicated to use.

#[allow(unused_imports)]
pub use platform::cleanup_window;
#[allow(unused_imports)]
pub use platform::fill_window;
#[allow(unused_imports)]
pub use platform::fill_window_with_animated_color;
#[allow(unused_imports)]
pub use platform::fill_window_with_border;
#[allow(unused_imports)]
pub use platform::fill_window_with_color;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
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
        context: RefCell<Context<&'static dyn Window>>,

        /// The hash map of window IDs to surfaces.
        surfaces: HashMap<WindowId, Surface<&'static dyn Window, &'static dyn Window>>,
    }

    impl GraphicsContext {
        fn new(w: &dyn Window) -> Self {
            Self {
                context: RefCell::new(
                    Context::new(unsafe {
                        mem::transmute::<&'_ dyn Window, &'static dyn Window>(w)
                    })
                    .expect("Failed to create a softbuffer context"),
                ),
                surfaces: HashMap::new(),
            }
        }

        fn create_surface(
            &mut self,
            window: &dyn Window,
        ) -> &mut Surface<&'static dyn Window, &'static dyn Window> {
            self.surfaces.entry(window.id()).or_insert_with(|| {
                Surface::new(&self.context.borrow(), unsafe {
                    mem::transmute::<&'_ dyn Window, &'static dyn Window>(window)
                })
                .expect("Failed to create a softbuffer surface")
            })
        }

        fn destroy_surface(&mut self, window: &dyn Window) {
            self.surfaces.remove(&window.id());
        }
    }

    pub fn fill_window_ex<F: Fn(usize, usize, &mut Buffer<&dyn Window, &dyn Window>)>(
        window: &dyn Window,
        f: F,
    ) {
        GC.with(|gc| {
            let size = window.surface_size();
            let (Some(width), Some(height)) =
                (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
            else {
                return;
            };

            // Either get the last context used or create a new one.
            let mut gc = gc.borrow_mut();
            let surface =
                gc.get_or_insert_with(|| GraphicsContext::new(window)).create_surface(window);

            // Fill a buffer with a solid color

            surface.resize(width, height).expect("Failed to resize the softbuffer surface");

            let mut buffer = surface.buffer_mut().expect("Failed to get the softbuffer buffer");

            f(size.width as usize, size.height as usize, &mut buffer);

            buffer.present().expect("Failed to present the softbuffer buffer");
        })
    }

    const DARK_GRAY: u32 = 0xff181818;
    const LEMON: u32 = 0xffd1ffbd;

    pub fn fill_window_with_color(window: &dyn Window, color: u32) {
        fill_window_ex(window, |_, _, buffer| buffer.fill(color));
    }

    #[allow(dead_code)]
    pub fn fill_window_with_border(window: &dyn Window) {
        fill_window_ex(window, |width, height, buffer| {
            for y in 0..height {
                for x in 0..width {
                    let color = if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                        LEMON
                    } else {
                        DARK_GRAY
                    };
                    buffer[y * width + x] = color;
                }
            }
        })
    }

    #[allow(dead_code)]
    pub fn fill_window(window: &dyn Window) {
        fill_window_with_color(window, DARK_GRAY);
    }

    #[allow(dead_code)]
    pub fn fill_window_with_animated_color(window: &dyn Window, start: std::time::Instant) {
        let time = start.elapsed().as_secs_f32() * 1.5;
        let blue = (time.sin() * 255.0) as u32;
        let green = ((time.cos() * 255.0) as u32) << 8;
        let red = ((1.0 - time.sin() * 255.0) as u32) << 16;
        let color = red | green | blue;
        fill_window_with_color(window, color);
    }

    #[allow(dead_code)]
    pub fn cleanup_window(window: &dyn Window) {
        GC.with(|gc| {
            let mut gc = gc.borrow_mut();
            if let Some(context) = gc.as_mut() {
                context.destroy_surface(window);
            }
        });
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
mod platform {
    #[allow(dead_code)]
    pub fn fill_window(_window: &dyn winit::window::Window) {
        // No-op on mobile platforms.
    }

    #[allow(dead_code)]
    pub fn fill_window_with_border(_window: &dyn winit::window::Window) {
        // No-op on mobile platforms.
    }

    #[allow(dead_code)]
    pub fn fill_window_with_color(_window: &dyn winit::window::Window, _color: u32) {
        // No-op on mobile platforms.
    }

    #[allow(dead_code)]
    pub fn fill_window_with_animated_color(
        _window: &dyn winit::window::Window,
        _start: std::time::Instant,
    ) {
        // No-op on mobile platforms.
    }

    #[allow(dead_code)]
    pub fn cleanup_window(_window: &dyn winit::window::Window) {
        // No-op on mobile platforms.
    }
}
