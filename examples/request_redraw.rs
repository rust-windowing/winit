extern crate winit;
use std::time::{Instant, Duration};

use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let events_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    events_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::EventsCleared => {
                window.request_redraw();
                *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::new(1, 0))
            },
            _ => ()
        }
    });
}
