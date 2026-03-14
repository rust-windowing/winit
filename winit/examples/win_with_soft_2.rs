use std::error::Error;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use softbuffer::{Context, Pixel, Surface};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

struct App {
    window: Option<Arc<dyn Window>>,
    surface: Option<Surface<Arc<dyn Window>, Arc<dyn Window>>>,
    start_time: Instant,
    frame_count: u32,
    last_fps_print: Instant,
    total_frame_count: u64,
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window: Arc<dyn Window> =
            Arc::from(event_loop.create_window(WindowAttributes::default()).unwrap());
        self.surface =
            Some(Surface::new(&Context::new(window.clone()).unwrap(), window.clone()).unwrap());
        self.window = Some(window);
    }

    fn about_to_wait(&mut self, _: &dyn ActiveEventLoop) {
        if let Some(w) = self.window.as_ref() {
            w.request_redraw();
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
                if let Some(s) = self.surface.as_mut() {
                    s.resize(
                        NonZeroU32::new(size.width.max(1)).unwrap(),
                        NonZeroU32::new(size.height.max(1)).unwrap(),
                    )
                    .unwrap();
                }
            },
            WindowEvent::RedrawRequested => {
                if let (Some(window), Some(surface)) = (&self.window, self.surface.as_mut()) {
                    let mut buffer = surface.next_buffer().unwrap();
                    let size = window.outer_size();
                    let elapsed = self.start_time.elapsed();
                    let t = elapsed.as_secs_f32();

                    let ball_x = ((t * 1.5).sin() * 0.5 + 0.5) * size.width as f32;
                    let ball_y = ((t * 2.1).cos() * 0.5 + 0.5) * size.height as f32;

                    for (x, y, pixel) in buffer.pixels_iter() {
                        let dx = x as f32 - ball_x;
                        let dy = y as f32 - ball_y;
                        let dist = (dx * dx + dy * dy).sqrt();

                        let p1 = (x as f32 * 0.01 + t).sin();
                        let p2 = (y as f32 * 0.01 + t * 0.5).cos();
                        let p3 = ((x as f32 + y as f32) * 0.005 + t).sin();
                        let plasma = (p1 + p2 + p3) * 0.33;

                        if dist < 50.0 {
                            *pixel = Pixel::new_rgb(255, 255, 255);
                        } else if (y as f32 + t * 50.0) % 20.0 < 2.0 {
                            *pixel = Pixel::new_rgb(0, 0, 0);
                        } else {
                            let r = ((plasma + 1.0) * 127.0) as u8;
                            let g = (x % 255) as u8;
                            let b = (y % 255).wrapping_add((t * 20.0) as u32) as u8;
                            *pixel = Pixel::new_rgb(r, g, b);
                        }
                    }
                    buffer.present().unwrap();

                    self.frame_count += 1;
                    self.total_frame_count += 1;
                    if self.last_fps_print.elapsed() >= Duration::from_secs(1) {
                        let avg = self.total_frame_count as f64 / elapsed.as_secs_f64();
                        window.set_title(&format!(
                            "FPS: {} | AVG: {:.2} | {}x{}",
                            self.frame_count, avg, size.width, size.height
                        ));
                        self.frame_count = 0;
                        self.last_fps_print = Instant::now();
                    }
                }
            },
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App {
        window: None,
        surface: None,
        start_time: Instant::now(),
        frame_count: 0,
        last_fps_print: Instant::now(),
        total_frame_count: 0,
    };
    EventLoop::new()?.run_app(app).map_err(Into::into)
}
