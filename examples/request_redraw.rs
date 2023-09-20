#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, elwt| {
        println!("{event:?}");

        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    ..
                } => {
                    window.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    println!("\nredrawing!\n");
                    fill::fill_window(&window);
                }
                _ => (),
            }
        }
    })
}
