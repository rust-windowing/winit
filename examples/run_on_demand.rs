#![allow(clippy::single_match)]

// Limit this example to only compatible platforms.
#[cfg(any(windows_platform, macos_platform, x11_platform, wayland_platform,))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Duration;

    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
    use winit::window::{Window, WindowAttributes, WindowId};

    #[path = "util/fill.rs"]
    mod fill;

    #[derive(Default)]
    struct App {
        idx: usize,
        window_id: Option<WindowId>,
        window: Option<Box<dyn Window>>,
    }

    impl App {
        fn init_window(&mut self, event_loop: &dyn ActiveEventLoop) {
            let window_attributes = WindowAttributes::default()
                .with_title("Fantastic window number one!")
                .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0));
            let window = event_loop.create_window(window_attributes).unwrap();
            self.window_id = Some(window.id());
            self.window = Some(window);
        }
    }

    impl ApplicationHandler for App {
        fn about_to_wait(&mut self, _event_loop: &dyn ActiveEventLoop) {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        fn window_event(
            &mut self,
            event_loop: &dyn ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            if event == WindowEvent::Destroyed && self.window_id == Some(window_id) {
                println!(
                    "--------------------------------------------------------- Window {} Destroyed",
                    self.idx
                );
                self.window_id = None;
                event_loop.exit();
                return;
            }

            let window = match self.window.as_mut() {
                Some(window) => window,
                None => return,
            };

            match event {
                WindowEvent::CloseRequested => {
                    println!(
                        "--------------------------------------------------------- Window {} \
                         CloseRequested",
                        self.idx
                    );
                    fill::cleanup_window(window.as_ref());
                    self.window = None;
                },
                WindowEvent::RedrawRequested => {
                    fill::fill_window(window.as_ref());
                },
                _ => (),
            }
        }
    }

    tracing_subscriber::fmt::init();

    let mut event_loop = EventLoop::new().unwrap();

    let mut app = App { idx: 1, ..Default::default() };
    event_loop.run_on_demand(|event_loop| {
        app.init_window(event_loop);
        &mut app
    })?;

    println!("--------------------------------------------------------- Finished first loop");
    println!("--------------------------------------------------------- Waiting 5 seconds");
    std::thread::sleep(Duration::from_secs(5));

    app.idx += 1;
    event_loop.run_on_demand(|event_loop| {
        app.init_window(event_loop);
        &mut app
    })?;
    println!("--------------------------------------------------------- Finished second loop");
    Ok(())
}

#[cfg(not(any(windows_platform, macos_platform, x11_platform, wayland_platform,)))]
fn main() {
    println!("This example is not supported on this platform");
}
