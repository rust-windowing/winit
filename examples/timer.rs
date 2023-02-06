#![allow(clippy::single_match)]

include!("it_util/timeout.rs");

use instant::Instant;
use std::time::Duration;

use simple_logger::SimpleLogger;
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();
    util::start_timeout_thread(&event_loop, ());

    let _window = WindowBuilder::new()
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
            }
            | Event::UserEvent(()) => control_flow.set_exit(),
            _ => (),
        }
    });
}
