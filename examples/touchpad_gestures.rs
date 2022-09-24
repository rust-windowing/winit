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
        .with_title("Touchpad gestures")
        .build(&event_loop)
        .unwrap();

    println!(
        r"
        Zoom is supported on both Windows and macOS,
        while rotation is only supported on Macos for now."
    );

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::TouchpadMagnify { delta, .. } => {
                    if delta > 0.0 {
                        println!("Zoomed in {}", delta);
                    } else {
                        println!("Zoomed out {}", delta);
                    }
                }
                WindowEvent::TouchpadRotate { delta, .. } => {
                    if delta > 0.0 {
                        println!("Rotated counterclockwise {}", delta);
                    } else {
                        println!("Rotated clockwise {}", delta);
                    }
                }
                _ => (),
            }
        }
    });
}
