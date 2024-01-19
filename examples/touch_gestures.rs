use simple_logger::SimpleLogger;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
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
    #[cfg(target_os = "ios")]
    {
        use winit::platform::ios::WindowExtIOS;
        window.recognize_doubletap_gesture(true);
        window.recognize_pinch_gesture(true);
        window.recognize_rotation_gesture(true);
    }

    println!("Only supported on macOS/iOS at the moment.");

    let mut zoom = 0.0;
    let mut rotated = 0.0;

    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::PinchGesture { delta, .. } => {
                    zoom += delta;
                    if delta > 0.0 {
                        println!("Zoomed in {delta:.5} (now: {zoom:.5})");
                    } else {
                        println!("Zoomed out {delta:.5} (now: {zoom:.5})");
                    }
                }
                WindowEvent::DoubleTapGesture { .. } => {
                    println!("Smart zoom");
                }
                WindowEvent::RotationGesture { delta, .. } => {
                    rotated += delta;
                    if delta > 0.0 {
                        println!("Rotated counterclockwise {delta:.5} (now: {rotated:.5})");
                    } else {
                        println!("Rotated clockwise {delta:.5} (now: {rotated:.5})");
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
