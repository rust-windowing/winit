use std::error::Error;

use softbuffer::{Context, Surface};
use tracing::info;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle};
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

fn main() -> Result<(), Box<dyn Error>> {
    tracing::init();

    let event_loop = EventLoop::new()?;

    let app = Application::default();
    Ok(event_loop.run_app(app)?)
}

/// Application state and event handling.
#[derive(Default, Debug)]
struct Application {
    surface: Option<Surface<OwnedDisplayHandle, Box<dyn Window>>>,
}

impl ApplicationHandler for Application {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window_attributes =
            WindowAttributes::default().with_title("Drag and drop files on me!");
        let window = event_loop.create_window(window_attributes).unwrap();
        let context = Context::new(event_loop.owned_display_handle()).unwrap();
        let surface = Surface::new(&context, window).unwrap();
        self.surface = Some(surface);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::DragLeft { .. }
            | WindowEvent::DragEntered { .. }
            | WindowEvent::DragMoved { .. }
            | WindowEvent::DragDropped { .. } => {
                info!("{event:?}");
            },
            WindowEvent::RedrawRequested => {
                let surface = self.surface.as_mut().unwrap();
                surface.window().pre_present_notify();
                fill::fill(surface);
            },
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            _ => {},
        }
    }
}
