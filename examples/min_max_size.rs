extern crate winit;

use winit::dpi::LogicalSize;
use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .build(&event_loop)
        .unwrap();

    window.set_min_dimensions(Some(LogicalSize::new(400.0, 200.0)));
    window.set_max_dimensions(Some(LogicalSize::new(800.0, 400.0)));

    event_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } =>
                *control_flow = ControlFlow::Exit,
            _ => *control_flow = ControlFlow::Wait,
        }
    });
}
