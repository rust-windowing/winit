#![allow(clippy::single_match)]

// Limit this example to only compatible platforms.
#[cfg(any(windows_platform, macos_platform, x11_platform, wayland_platform,))]
fn main() -> Result<(), impl std::error::Error> {
    use std::time::Duration;

    use simple_logger::SimpleLogger;

    use winit::{
        error::EventLoopError,
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        platform::run_on_demand::EventLoopExtRunOnDemand,
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
    let mut event_loop = EventLoop::new().unwrap();

    fn run_app(event_loop: &mut EventLoop<()>, idx: usize) -> Result<(), EventLoopError> {
        let mut app = App::default();

        event_loop.run_on_demand(move |event, elwt| {
            println!("Run {idx}: {:?}", event);

            if let Some(window) = &app.window {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        window_id,
                    } if window.id() == window_id => {
                        println!("--------------------------------------------------------- Window {idx} CloseRequested");
                        fill::cleanup_window(window);
                        app.window = None;
                    }
                    Event::AboutToWait => window.request_redraw(),
                    Event::WindowEvent {
                        event: WindowEvent::RedrawRequested,
                        ..
                    }  => {
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
                        elwt.exit();
                    }
                    _ => (),
                }
            } else if let Event::Resumed = event {
                let window = WindowBuilder::new()
                        .with_title("Fantastic window number one!")
                        .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
                        .build(elwt)
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
