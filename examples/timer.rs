extern crate winit;
use std::time::{Duration, Instant};
use winit::{Event, WindowEvent, StartCause, ControlFlow};

fn main() {
    let events_loop = winit::EventLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    events_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        match event {
            Event::NewEvents(StartCause::Init) =>
                *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::new(1, 0)),
            Event::NewEvents(StartCause::ResumeTimeReached{..}) => {
                *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::new(1, 0));
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
