//! Simple winit window example.

use std::error::Error;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
#[cfg(web_platform)]
use winit::platform::web::WindowAttributesExtWeb;
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;

#[derive(Debug)]
struct App {
    window: Option<Box<dyn Window>>,
    start_time: std::time::Instant,
    continuous_redraw: bool,
}

impl App {
    fn new(continuous_redraw: bool) -> App {
        App { window: None, start_time: std::time::Instant::now(), continuous_redraw }
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        #[cfg(not(web_platform))]
        let window_attributes = WindowAttributes::default();
        #[cfg(web_platform)]
        let window_attributes = WindowAttributes::default().with_append(true);
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
        println!("{event:?}");
        match event {
            WindowEvent::CloseRequested => {
                println!("Close was requested; stopping");
                event_loop.exit();
            },
            WindowEvent::SurfaceResized(_) => {
                self.window.as_ref().expect("resize event without a window").request_redraw();
            },
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                let window = self.window.as_ref().expect("redraw request without a window");

                // Notify that you're about to draw.
                window.pre_present_notify();

                if self.continuous_redraw {
                    // Animate the fill color. This may be used to demonstrate smooth window
                    // resizing and movement when interacting with the window
                    // frame or title bar.
                    fill::fill_window_with_animated_color(window.as_ref(), self.start_time);

                    // For contiguous redraw loop you can request a redraw from here.
                    window.request_redraw();
                } else {
                    // Draw.
                    fill::fill_window(window.as_ref());
                }
            },
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    let event_loop = EventLoop::new()?;

    // Set to true to continuously redraw the window which will also animate the fill color.
    let continuous_redraw = false;

    let mut app = App::new(continuous_redraw);

    // For alternative loop run options see `pump_events` and `run_on_demand` examples.
    event_loop.run_app(&mut app).map_err(Into::into)
}
