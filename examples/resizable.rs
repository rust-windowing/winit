use simple_logger::SimpleLogger;
use winit::{
    dpi::LogicalSize,
    event::{
        keyboard_types::{Code, KeyState},
        Event, KeyEvent, WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let mut resizable = false;

    let window = WindowBuilder::new()
        .with_title("Hit space to toggle resizability.")
        .with_inner_size(LogicalSize::new(400.0, 200.0))
        .with_resizable(resizable)
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: Code::Space,
                            state: KeyState::Up,
                            ..
                        },
                    ..
                } => {
                    resizable = !resizable;
                    println!("Resizable: {}", resizable);
                    window.set_resizable(resizable);
                }
                _ => (),
            },
            _ => (),
        };
    });
}
