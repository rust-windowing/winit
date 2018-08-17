extern crate winit;
use std::time::{Duration, Instant};

fn main() {
    let events_loop = winit::EventLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    events_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        match event {
            winit::Event::NewEvents(winit::StartCause::Init) =>
                *control_flow = winit::ControlFlow::WaitTimeout(Duration::new(1, 0)),
            winit::Event::NewEvents(winit::StartCause::TimeoutExpired{..}) => {
                *control_flow = winit::ControlFlow::WaitTimeout(Duration::new(1, 0));
                println!("\nTimer\n");
            },
            winit::Event::NewEvents(winit::StartCause::WaitCancelled{start, requested_duration}) => {
                println!("{:?}", Instant::now() - start);
                *control_flow = winit::ControlFlow::WaitTimeout(requested_duration.unwrap().checked_sub(Instant::now() - start).unwrap_or(Duration::new(0, 0)));
            }
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                ..
            } => *control_flow = winit::ControlFlow::Exit,
            _ => ()
        }
    });
}
