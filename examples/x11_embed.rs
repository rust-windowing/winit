//! A demonstration of embedding a winit window in an existing X11 application.

#[cfg(x11_platform)]
#[path = "util/fill.rs"]
mod fill;

#[cfg(x11_platform)]
mod imple {
    use super::fill;
    use simple_logger::SimpleLogger;
    use winit::{
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        platform::x11::WindowBuilderExtX11,
        window::WindowBuilder,
    };

    pub(super) fn entry() -> Result<(), Box<dyn std::error::Error>> {
        // First argument should be a 32-bit X11 window ID.
        let parent_window_id = std::env::args()
            .nth(1)
            .ok_or("Expected a 32-bit X11 window ID as the first argument.")?
            .parse::<u32>()?;

        SimpleLogger::new().init().unwrap();
        let event_loop = EventLoop::new()?;

        let window = WindowBuilder::new()
            .with_title("An embedded window!")
            .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
            .with_embed_parent_window(parent_window_id)
            .build(&event_loop)
            .unwrap();

        event_loop.run(move |event, elwt| {
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id,
                } if window_id == window.id() => elwt.exit(),
                Event::AboutToWait => {
                    window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    // Notify the windowing system that we'll be presenting to the window.
                    window.pre_present_notify();
                    fill::fill_window(&window);
                }
                _ => (),
            }
        })?;

        Ok(())
    }
}

#[cfg(not(x11_platform))]
mod imple {
    pub(super) fn entry() -> Result<(), Box<dyn std::error::Error>> {
        println!("This example is only supported on X11 platforms.");
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    imple::entry()
}
