//! Demonstrates plugging an external [`EventLoopProvider`] into [`EventLoop::from_backend`].
//!
//! Because [`EventLoop`] implements [`EventLoopProvider`] itself, this example plugs a built-in
//! `EventLoop` into another `EventLoop` without needing a separate backend crate.

use std::error::Error;

use softbuffer::{Context, Surface};
use tracing::info;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle};
#[cfg(web_platform)]
use winit::platform::web::WindowAttributesWeb;
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

#[derive(Default, Debug)]
struct App {
    surface: Option<Surface<OwnedDisplayHandle, Box<dyn Window>>>,
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        #[cfg(not(web_platform))]
        let window_attributes =
            WindowAttributes::default().with_title("Winit custom backend example");
        #[cfg(web_platform)]
        let window_attributes = WindowAttributes::default()
            .with_title("Winit custom backend example")
            .with_platform_attributes(Box::new(WindowAttributesWeb::default().with_append(true)));
        let window = event_loop.create_window(window_attributes).expect("failed creating window");

        let context =
            Context::new(event_loop.owned_display_handle()).expect("failed creating context");
        let surface = Surface::new(&context, window).expect("failed creating surface");
        self.surface = Some(surface);
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        info!("{event:?}");
        match event {
            WindowEvent::CloseRequested => {
                info!("Close was requested; stopping");
                event_loop.exit();
            },
            WindowEvent::SurfaceResized(surface_size) => {
                let surface = self.surface.as_mut().expect("resize event without a surface");
                fill::resize(surface, surface_size);
                surface.window().request_redraw();
            },
            WindowEvent::RedrawRequested => {
                let surface = self.surface.as_mut().expect("redraw event without a surface");
                surface.window().pre_present_notify();

                let mut buffer = surface.buffer_mut().expect("Failed to get the softbuffer buffer");
                buffer.fill(0xff181818);
                buffer.present().expect("Failed to present the softbuffer buffer");
            },
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    tracing::init();

    let backend = EventLoop::new()?;
    let event_loop = EventLoop::from_backend(Box::new(backend));
    assert!(event_loop.backend().cast_ref::<EventLoop>().is_some());

    event_loop.run_app(Box::new(App::default()))?;

    Ok(())
}
