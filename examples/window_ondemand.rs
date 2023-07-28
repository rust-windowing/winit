#![allow(clippy::single_match)]

// Limit this example to only compatible platforms.
#[cfg(any(windows_platform, macos_platform, x11_platform, wayland_platform,))]
fn main() -> Result<(), impl std::error::Error> {
    use std::time::Duration;

    use simple_logger::SimpleLogger;

    use winit::{
        error::RunLoopError,
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        platform::run_ondemand::EventLoopExtRunOnDemand,
        window::{Window, WindowBuilder, WindowId},
    };

    #[path = "util/fill.rs"]
    mod fill;

    #[derive(Default)]
    struct App {
        window_id: Option<WindowId>,
        window: Option<Window>,
    }

    SimpleLogger::new().init().unwrap();
    let mut event_loop = EventLoop::new();

    fn run_app(event_loop: &mut EventLoop<()>, idx: usize) -> Result<(), RunLoopError> {
        let mut app = App::default();

        event_loop.run_ondemand(move |event, event_loop, control_flow| {
            control_flow.set_wait();
            println!("Run {idx}: {:?}", event);

            if let Some(window) = &app.window {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        window_id,
                    } if window.id() == window_id => {
                        println!("--------------------------------------------------------- Window {idx} CloseRequested");
                        app.window = None;
                    }
                    Event::MainEventsCleared => window.request_redraw(),
                    Event::RedrawRequested(_) => {
                        fill::fill_window(window);
                    }
                    _ => (),
                }
            } else if let Some(id) = app.window_id {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::Destroyed,
                        window_id,
                    } if id == window_id => {
                        println!("--------------------------------------------------------- Window {idx} Destroyed");
                        app.window_id = None;
                        control_flow.set_exit();
                    }
                    _ => (),
                }
            } else if let Event::Resumed = event {
                let window = WindowBuilder::new()
                        .with_title("Fantastic window number one!")
                        .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
                        .build(event_loop)
                        .unwrap();
                app.window_id = Some(window.id());
                app.window = Some(window);
            }
        })
    }

    run_app(&mut event_loop, 1)?;

    println!("--------------------------------------------------------- Finished first loop");
    println!("--------------------------------------------------------- Waiting 5 seconds");
    std::thread::sleep(Duration::from_secs(5));

    let ret = run_app(&mut event_loop, 2);
    println!("--------------------------------------------------------- Finished second loop");
    ret
}

#[cfg(not(any(windows_platform, macos_platform, x11_platform, wayland_platform,)))]
fn main() {
    println!("This example is not supported on this platform");
}
