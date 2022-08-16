#![allow(clippy::single_match)]

// Limit this example to only compatible platforms.
#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "android",
))]
fn main() {
    use std::{thread::sleep, time::Duration};

    use simple_logger::SimpleLogger;
    use winit::{
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        platform::run_return::EventLoopExtRunReturn,
        window::WindowBuilder,
    };
    let mut event_loop = EventLoop::new();

    SimpleLogger::new().init().unwrap();
    let _window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    let mut quit = false;

    while !quit {
        event_loop.run_return(|event, _, control_flow| {
            control_flow.set_wait();

            if let Event::WindowEvent { event, .. } = &event {
                // Print only Window events to reduce noise
                println!("{:?}", event);
            }

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    quit = true;
                }
                Event::MainEventsCleared => {
                    control_flow.set_exit();
                }
                _ => (),
            }
        });

        // Sleep for 1/60 second to simulate rendering
        println!("rendering");
        sleep(Duration::from_millis(16));
    }
}

#[cfg(any(target_os = "ios", target_arch = "wasm32"))]
fn main() {
    println!("This platform doesn't support run_return.");
}
