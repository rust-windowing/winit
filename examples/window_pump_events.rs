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
        window::WindowBuilder,
    };

    #[path = "util/fill.rs"]
    mod fill;

    let mut event_loop = EventLoop::new().unwrap();

    SimpleLogger::new().init().unwrap();
    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    'main: loop {
        let timeout = Some(Duration::ZERO);
        let status = event_loop.pump_events(timeout, |event, elwt| {
            if let Event::WindowEvent { event, .. } = &event {
                // Print only Window events to reduce noise
                println!("{event:?}");
            }

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id,
                } if window_id == window.id() => elwt.exit(),
                Event::AboutToWait => {
                    window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    fill::fill_window(&window);
                }
                _ => (),
            }
        });
        if let PumpStatus::Exit(exit_code) = status {
            break 'main ExitCode::from(exit_code as u8);
        }

        // Sleep for 1/60 second to simulate application work
        //
        // Since `pump_events` doesn't block it will be important to
        // throttle the loop in the app somehow.
        println!("Update()");
        sleep(Duration::from_millis(16));
    }
}

#[cfg(any(ios_platform, wasm_platform, orbital_platform))]
fn main() {
    println!("This platform doesn't support pump_events.");
}
