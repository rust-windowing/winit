#![allow(clippy::single_match)]

// Limit this example to only compatible platforms.
#[cfg(any(windows_platform, macos_platform, x11_platform, wayland_platform, orbital_platform))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Duration;

    use softbuffer::{Context, Surface};
    use tracing::info;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::run_on_demand::EventLoopExtRunOnDemand;
    use winit::event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle};
    use winit::window::{Window, WindowAttributes, WindowId};

    #[path = "util/fill.rs"]
    mod fill;
    #[path = "util/tracing.rs"]
    mod tracing;

    #[derive(Debug)]
    struct App {
        context: Context<OwnedDisplayHandle>,
        idx: usize,
        surface: Option<Surface<OwnedDisplayHandle, Box<dyn Window>>>,
        window_id: Option<WindowId>,
    }

    impl ApplicationHandler for App {
        fn about_to_wait(&mut self, _event_loop: &dyn ActiveEventLoop) {
            if let Some(surface) = self.surface.as_ref() {
                surface.window().request_redraw();
            }
        }

        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            let window_attributes = WindowAttributes::default()
                .with_title(format!("Fantastic window number {}!", self.idx))
                .with_surface_size(winit::dpi::LogicalSize::new(128.0, 128.0));
            let window = event_loop.create_window(window_attributes).unwrap();
            self.window_id = Some(window.id());

            let surface = Surface::new(&self.context, window).unwrap();
            self.surface = Some(surface);
        }

        fn window_event(
            &mut self,
            event_loop: &dyn ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            if event == WindowEvent::Destroyed && self.window_id == Some(window_id) {
                info!("Window {} Destroyed", self.idx);
                self.window_id = None;
                event_loop.exit();
                return;
            }

            let Some(surface) = self.surface.as_mut() else {
                return;
            };

            match event {
                WindowEvent::CloseRequested => {
                    info!("Window {} CloseRequested", self.idx);
                    self.surface = None;
                },
                WindowEvent::RedrawRequested => {
                    fill::fill(surface);
                },
                _ => (),
            }
        }
    }

    tracing::init();

    let mut event_loop = EventLoop::new().unwrap();

    let context = Context::new(event_loop.owned_display_handle()).unwrap();
    let mut app = App { context, idx: 1, surface: None, window_id: None };
    event_loop.run_app_on_demand(&mut app)?;

    info!("Finished first loop");
    info!("Waiting 5 seconds");
    std::thread::sleep(Duration::from_secs(5));

    app.idx += 1;
    event_loop.run_app_on_demand(&mut app)?;
    info!("Finished second loop");
    Ok(())
}

#[cfg(not(any(
    windows_platform,
    macos_platform,
    x11_platform,
    wayland_platform,
    orbital_platform
)))]
fn main() {
    panic!("This example is not supported on this platform")
}
