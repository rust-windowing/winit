use std::error::Error;
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use softbuffer::{Context, Surface};

struct App {
    window: Option<Arc<dyn Window>>,
    surface: Option<Surface<Arc<dyn Window>, Arc<dyn Window>>>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            surface: None,
        }
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let attributes = WindowAttributes::default().with_title("Framebuffer Example");
        
        let window: Arc<dyn Window> = Arc::from(event_loop.create_window(attributes).unwrap());

        let context = Context::new(window.clone()).expect("context except");
        let surface = Surface::new(&context, window.clone()).expect("surface except");

        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            
            WindowEvent::SurfaceResized(size) => {
                if let Some(surface) = self.surface.as_mut() {
                    surface.resize(
                        NonZeroU32::new(size.width.max(1)).unwrap(),
                        NonZeroU32::new(size.height.max(1)).unwrap()
                    ).unwrap();
                }
            }

            WindowEvent::RedrawRequested => {
                if let (Some(window), Some(surface)) = (self.window.as_ref(), self.surface.as_mut()) {
                    let size = window.outer_size(); 
                    
                    let mut buffer = surface.buffer_mut().expect("buffer except");

                    let width = size.width as usize;
                    for (index, pixel) in buffer.iter_mut().enumerate() {
                        let x = index % width;
                        let y = index / width;
                        let r = (x ^ y) as u32 & 0xFF;
                        let g = x as u32 % 255;
                        let b = 128;
                        *pixel = b | (g << 8) | (r << 16);
                    }

                    buffer.present().unwrap();
                }
            }
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