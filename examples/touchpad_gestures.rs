use simple_logger::SimpleLogger;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("Touchpad gestures")
        .build(&event_loop)
        .unwrap();

    println!("Only supported on macOS at the moment.");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::TouchpadMagnify { delta, .. } => {
                    if delta > 0.0 {
                        println!("Zoomed in {delta}");
                    } else {
                        println!("Zoomed out {delta}");
                    }
                }
                WindowEvent::SmartMagnify { .. } => {
                    println!("Smart zoom");
                }
                WindowEvent::TouchpadRotate { delta, .. } => {
                    if delta > 0.0 {
                        println!("Rotated counterclockwise {delta}");
                    } else {
                        println!("Rotated clockwise {delta}");
                    }
                }
                WindowEvent::RedrawRequested => {
                    fill::fill_window(&window);
                }
                _ => (),
            }
        }
    })
}
