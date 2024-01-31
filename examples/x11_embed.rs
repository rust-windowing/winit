//! A demonstration of embedding a winit window in an existing X11 application.
use std::error::Error;

#[cfg(x11_platform)]
fn main() -> Result<(), Box<dyn Error>> {
    use simple_logger::SimpleLogger;

    use winit::{
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        platform::x11::WindowAttributesExtX11,
        window::Window,
    };

    #[path = "util/fill.rs"]
    mod fill;

    // First argument should be a 32-bit X11 window ID.
    let parent_window_id = std::env::args()
        .nth(1)
        .ok_or("Expected a 32-bit X11 window ID as the first argument.")?
        .parse::<u32>()?;

    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new()?;

    let mut window = None;
    event_loop.run(move |event, event_loop| match event {
        Event::Resumed => {
            let window_attributes = Window::default_attributes()
                .with_title("An embedded window!")
                .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
                .with_embed_parent_window(parent_window_id);

            window = Some(event_loop.create_window(window_attributes).unwrap());
        }
        Event::WindowEvent { event, .. } => {
            let window = window.as_ref().unwrap();

            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => {
                    window.pre_present_notify();
                    fill::fill_window(window);
                }
                _ => (),
            }
        }
        Event::AboutToWait => {
            window.as_ref().unwrap().request_redraw();
        }
        _ => (),
    })?;

    Ok(())
}

#[cfg(not(x11_platform))]
fn main() -> Result<(), Box<dyn Error>> {
    println!("This example is only supported on X11 platforms.");
    Ok(())
}
