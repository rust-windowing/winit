use simple_logger::SimpleLogger;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("Touchpad magnify events")
        .build(&event_loop)
        .unwrap();

    println!("Only supported on macOS at the moment.");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::TouchpadMagnify { delta, .. } => {
                    if delta > 0.0 {
                        println!("Zoomed in {}", delta);
                    } else {
                        println!("Zoomed out {}", delta);
                    }
                },
                _ => (),
            },
            _ => (),
        }
    });
}
