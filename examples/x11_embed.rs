//! A demonstration of embedding a winit window in an existing X11 application.
use std::error::Error;

#[cfg(x11_platform)]
fn main() -> Result<(), Box<dyn Error>> {
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::platform::x11::WindowAttributesExtX11;
    use winit::window::{Window, WindowAttributes, WindowId};

    #[path = "util/fill.rs"]
    mod fill;

    pub struct XEmbedDemo {
        parent_window_id: u32,
        window: Option<Box<dyn Window>>,
    }

    impl ApplicationHandler for XEmbedDemo {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            let window_attributes = WindowAttributes::default()
                .with_title("An embedded window!")
                .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
                .with_embed_parent_window(self.parent_window_id);

            self.window = Some(event_loop.create_window(window_attributes).unwrap());
        }

        fn window_event(
            &mut self,
            event_loop: &dyn ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            let window = self.window.as_ref().unwrap();
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => {
                    window.pre_present_notify();
                    fill::fill_window(window.as_ref());
                },
                _ => (),
            }
        }

        fn about_to_wait(&mut self, _event_loop: &dyn ActiveEventLoop) {
            self.window.as_ref().unwrap().request_redraw();
        }
    }

    // First argument should be a 32-bit X11 window ID.
    let parent_window_id = std::env::args()
        .nth(1)
        .ok_or("Expected a 32-bit X11 window ID as the first argument.")?
        .parse::<u32>()?;

    tracing_subscriber::fmt::init();
    let event_loop = EventLoop::new()?;

    Ok(event_loop.run_app(XEmbedDemo { parent_window_id, window: None })?)
}

#[cfg(not(x11_platform))]
fn main() -> Result<(), Box<dyn Error>> {
    println!("This example is only supported on X11 platforms.");
    Ok(())
}
