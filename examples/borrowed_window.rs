//! Use borrowed window handles in `winit`.

#![allow(clippy::single_match)]

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use simple_logger::SimpleLogger;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A borrowed window!")
        .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, elwt, control_flow| {
        control_flow.set_wait();
        println!("{event:?}");

        // Print the display handle.
        println!("Display handle: {:?}", elwt.display_handle());

        // Print the window handle.
        match window.window_handle() {
            Ok(handle) => println!("Window handle: {:?}", handle),
            Err(_) => println!("Window handle: None"),
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => control_flow.set_exit(),
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => (),
        }
    });
}
