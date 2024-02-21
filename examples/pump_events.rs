#![allow(clippy::single_match)]

// Limit this example to only compatible platforms.
#[cfg(any(
    windows_platform,
    macos_platform,
    x11_platform,
    wayland_platform,
    android_platform,
))]
fn main() -> std::process::ExitCode {
    use std::{process::ExitCode, thread::sleep, time::Duration};

    use simple_logger::SimpleLogger;
    use winit::{
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        platform::pump_events::{EventLoopExtPumpEvents, PumpStatus},
        window::Window,
    };

    #[path = "util/fill.rs"]
    mod fill;

    let mut event_loop = EventLoop::new().unwrap();

    SimpleLogger::new().init().unwrap();

    let mut window = None;

    loop {
        let timeout = Some(Duration::ZERO);
        let status = event_loop.pump_events(timeout, |event, event_loop| {
            if let Event::WindowEvent { event, .. } = &event {
                // Print only Window events to reduce noise
                println!("{event:?}");
            }

            match event {
                Event::Resumed => {
                    let window_attributes =
                        Window::default_attributes().with_title("A fantastic window!");
                    window = Some(event_loop.create_window(window_attributes).unwrap());
                }
                Event::WindowEvent { event, .. } => {
                    let window = window.as_ref().unwrap();
                    match event {
                        WindowEvent::CloseRequested => event_loop.exit(),
                        WindowEvent::RedrawRequested => fill::fill_window(window),
                        _ => (),
                    }
                }
                Event::AboutToWait => {
                    window.as_ref().unwrap().request_redraw();
                }
                _ => (),
            }
        });

        if let PumpStatus::Exit(exit_code) = status {
            break ExitCode::from(exit_code as u8);
        }

        // Sleep for 1/60 second to simulate application work
        //
        // Since `pump_events` doesn't block it will be important to
        // throttle the loop in the app somehow.
        println!("Update()");
        sleep(Duration::from_millis(16));
    }
}

#[cfg(any(ios_platform, web_platform, orbital_platform))]
fn main() {
    println!("This platform doesn't support pump_events.");
}
