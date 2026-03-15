#[cfg(any(
    windows_platform,
    macos_platform,
    x11_platform,
    wayland_platform,
    orbital_platform
))]
use std::error::Error;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

struct App {
    window: Option<Arc<dyn Window>>,
    surface: Option<Surface<Arc<dyn Window>, Arc<dyn Window>>>,
    frame_count: u32,
    last_fps_print: Instant,
}

impl App {
    fn new() -> Self {
        Self { window: None, surface: None, frame_count: 0, last_fps_print: Instant::now() }
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let attributes = WindowAttributes::default().with_title("framebuffer test");
        let window: Arc<dyn Window> = Arc::from(event_loop.create_window(attributes).unwrap());

        let context = Context::new(window.clone()).expect("context except");
        let surface = Surface::new(&context, window.clone()).expect("surface except");

        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn about_to_wait(&mut self, _event_loop: &dyn ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::SurfaceResized(size) => {
                if let Some(surface) = self.surface.as_mut() {
                    let w = NonZeroU32::new(size.width.max(1)).unwrap();
                    let h = NonZeroU32::new(size.height.max(1)).unwrap();
                    surface.resize(w, h).unwrap();
                }
            },

            WindowEvent::RedrawRequested => {
                if let Some(surface) = self.surface.as_mut() {
                    let mut buffer = surface.next_buffer().expect("buffer except");

                    for (x, y, pixel) in buffer.pixels_iter() {
                        let red = (x % 255) as u8;
                        let green = (y % 255) as u8;
                        let blue = ((x * y) % 255) as u8;

                        *pixel = softbuffer::Pixel::new_rgb(red, green, blue);
                    }

                    buffer.present().unwrap();

                    let now = Instant::now();
                    self.frame_count += 1;

                    if now.duration_since(self.last_fps_print) >= Duration::from_secs(1) {
                        let fps = self.frame_count;
                        println!("FPS: {}", fps);

                        self.frame_count = 0;
                        self.last_fps_print = now;
                    }
                }
            },
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;

    let app = Box::leak(Box::new(App::new()));

    event_loop.run_app(app)?;

    Ok(())
}
