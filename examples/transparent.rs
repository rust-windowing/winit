#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::Window,
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = Window::builder()
        .with_decorations(false)
        .with_transparent(true)
        .build(&event_loop)
        .unwrap();

    window.set_title("A fantastic window!");

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();
        println!("{event:?}");

        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => control_flow.set_exit(),
                WindowEvent::RedrawRequested => {
                    fill::fill_window(&window);
                }
                _ => (),
            }
        }
    })
}
