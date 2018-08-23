extern crate winit;
use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let events_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    events_loop.run(|event, _, control_flow| {
        println!("{:?}", event);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => *control_flow = ControlFlow::Wait,
        }
    });
}
