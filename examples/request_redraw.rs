use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    ..
                } => {
                    window.request_redraw();
                }
                _ => (),
            },
            Event::RedrawRequested(_) => {
                println!("\nredrawing!\n");
            }
            _ => (),
        }
    });
}
