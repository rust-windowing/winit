#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::KeyCode,
    window::WindowBuilder,
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let mut resizable = false;

    let window = WindowBuilder::new()
        .with_title("Hit space to toggle resizability.")
        .with_inner_size(LogicalSize::new(600.0, 300.0))
        .with_min_inner_size(LogicalSize::new(400.0, 200.0))
        .with_max_inner_size(LogicalSize::new(800.0, 400.0))
        .with_resizable(resizable)
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => control_flow.set_exit(),
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: KeyCode::Space,
                            state: ElementState::Released,
                            ..
                        },
                    ..
                } => {
                    resizable = !resizable;
                    println!("Resizable: {resizable}");
                    window.set_resizable(resizable);
                }
                _ => (),
            },
            Event::RedrawRequested(_) => {
                fill::fill_window(&window);
            }
            _ => (),
        };
    })
}
