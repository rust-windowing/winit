#![allow(clippy::single_match)]

use std::time::Duration;
#[cfg(not(wasm_platform))]
use std::time::Instant;
#[cfg(wasm_platform)]
use web_time::Instant;

use simple_logger::SimpleLogger;
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    let timer_length = Duration::new(1, 0);

    event_loop.run(move |event, _, control_flow| {
        println!("{event:?}");

        match event {
            Event::NewEvents(StartCause::Init) => {
                control_flow.set_wait_until(Instant::now() + timer_length);
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                control_flow.set_wait_until(Instant::now() + timer_length);
                println!("\nTimer\n");
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => control_flow.set_exit(),
            Event::RedrawRequested(_) => {
                fill::fill_window(&window);
            }
            _ => (),
        }
    })
}
