extern crate winit;
use std::time::{Duration, Instant};
use winit::event::{Event, WindowEvent, StartCause};
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let event_loop = EventLoop::new();

    let timer_length = Duration::new(1, 0);

    event_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        match event {
            Event::NewEvents(StartCause::Init) =>
                *control_flow = ControlFlow::WaitUntil(Instant::now() + timer_length),
            Event::NewEvents(StartCause::ResumeTimeReached{..}) => {
                *control_flow = ControlFlow::WaitUntil(Instant::now() + timer_length);
                println!("\nTimer\n");
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => ()
        }
    });
}
