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
    totla_frame_count: u64,
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window: Arc<dyn Window> = Arc::from(event_loop.create_window(WindowAttributes::default()).unwrap());
        self.surface = Some(Surface::new(&Context::new(window.clone()).unwrap(), window.clone()).unwrap());
        self.window = Some(window);
    }

    fn about_to_wait(&mut self, _: &dyn ActiveEventLoop) {
        if let Some(w) = self.window.as_ref() { w.request_redraw(); }
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::SurfaceResized(size) => {
                if let Some(s) = self.surface.as_mut() {
                    s.resize(NonZeroU32::new(size.width.max(1)).unwrap(), NonZeroU32::new(size.height.max(1)).unwrap()).unwrap();
                }
            },
            WindowEvent::RedrawRequested => {
                if let (Some(window), Some(surface)) = (&self.window, self.surface.as_mut()) {
                    let mut buffer = surface.next_buffer().unwrap();
                    let size = window.outer_size();

                    let elapsed_total = self.start_time.elapsed();
                    let time = self.start_time.elapsed().as_secs_f32();

                    for (x, y, pixel) in buffer.pixels_iter() {
                        let wave = (time.sin() * 50.0) as u32;
                        let r = (x % 255) as u8;
                        let g = (y % 255).wrapping_add(wave as u32) as u8;
                        let b = (time * 100.0) as u8;

                        if y > size.height / 3 && y < size.height / 2 {
                            *pixel = Pixel::new_rgb(r.wrapping_add(b), g, 255);
                        } else {
                            *pixel = Pixel::new_rgb(r, g, b);
                        }
                    }
                    buffer.present().unwrap();

                    self.frame_count += 1;
                    self.totla_frame_count += 1;

                    if self.last_fps_print.elapsed() >= Duration::from_secs(1) {
                        let avg_fps = self.totla_frame_count as f64 / elapsed_total.as_secs_f64();
                        
                        window.set_title(&format!(
                            "FPS: {} | AVG: {:.2} | Size: {}x{}", 
                            self.frame_count, 
                            avg_fps, 
                            size.width, 
                            size.height
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
    let event_loop = EventLoop::new()?;
    let app = Box::leak(Box::new(App {
        window: None,
        surface: None,
        start_time: Instant::now(),
        frame_count: 0,
        last_fps_print: Instant::now(),
        totla_frame_count: 0
    }));
    event_loop.run_app(app).map_err(Into::into)
}