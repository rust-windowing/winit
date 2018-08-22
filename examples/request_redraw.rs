extern crate winit;
use winit::{Event, WindowEvent};
use std::time::{Instant, Duration};

fn main() {
    let events_loop = winit::EventLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    events_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = winit::ControlFlow::Exit,
            Event::EventsCleared => {
                window.request_redraw();
                *control_flow = winit::ControlFlow::WaitUntil(Instant::now() + Duration::new(1, 0))
            },
            _ => ()
        }
    });
}
