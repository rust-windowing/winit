#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let min_size = LogicalSize::new(400.0, 200.0);
    let max_size = LogicalSize::new(800.0, 400.0);
    let mut size = min_size;

    let window = Window::builder()
        .with_title("Hit space to toggle size.")
        .with_inner_size(size)
        .with_min_inner_size(min_size)
        .with_max_inner_size(max_size)
        .without_size_suggestions(true)
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(KeyCode::Space),
                            state: ElementState::Released,
                            ..
                        },
                    ..
                } => {
                    size = if size == min_size { max_size } else { min_size };
                    println!("New size: {:?}", size);
                    let _ = window.request_inner_size(size);
                }
                WindowEvent::RedrawRequested => {
                    fill::fill_window(&window);
                }
                _ => (),
            }
        };
    })
}
