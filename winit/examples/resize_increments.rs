//! Example of window resize increments.

use std::error::Error;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

#[derive(Default, Debug)]
struct App {
    window: Option<Box<dyn Window>>,
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window_attributes = WindowAttributes::default()
            .with_title("Resize Increments Test")
            .with_surface_resize_increments(LogicalSize::new(50.0, 50.0));
            
        self.window = match event_loop.create_window(window_attributes) {
            Ok(window) => Some(window),
            Err(err) => {
                eprintln!("error creating window: {err}");
                event_loop.exit();
                return;
            },
        }
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            WindowEvent::SurfaceResized(_) => {
                self.window.as_ref().expect("resize event without a window").request_redraw();
            },
            WindowEvent::RedrawRequested => {
                let window = self.window.as_ref().expect("redraw request without a window");
                window.pre_present_notify();
                fill::fill_window(window.as_ref());
            },
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    tracing::init();
    let event_loop = EventLoop::new()?;
    event_loop.run_app(App::default())?;
    Ok(())
}
