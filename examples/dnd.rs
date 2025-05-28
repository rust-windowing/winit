use std::error::Error;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, SurfaceId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

fn main() -> Result<(), Box<dyn Error>> {
    tracing::init();

    let event_loop = EventLoop::new()?;

    let app = Application::new();
    Ok(event_loop.run_app(app)?)
}

/// Application state and event handling.
#[derive(Debug)]
struct Application {
    window: Option<Box<dyn Window>>,
}

impl Application {
    fn new() -> Self {
        Self { window: None }
    }
}

impl ApplicationHandler for Application {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window_attributes =
            WindowAttributes::default().with_title("Drag and drop files on me!");
        self.window = Some(event_loop.create_window(window_attributes).unwrap());
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _window_id: SurfaceId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::DragLeft { .. }
            | WindowEvent::DragEntered { .. }
            | WindowEvent::DragMoved { .. }
            | WindowEvent::DragDropped { .. } => {
                println!("{event:?}");
            },
            WindowEvent::RedrawRequested => {
                let window = self.window.as_ref().unwrap();
                window.pre_present_notify();
                fill::fill_window(window.as_ref());
            },
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            _ => {},
        }
    }
}
