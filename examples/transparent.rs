use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_decorations(false)
        .with_transparent(true)
        .build(&event_loop)
        .unwrap();

    window.set_title("A fantastic window!");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        println!("{:?}", event);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
