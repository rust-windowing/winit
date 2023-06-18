#![allow(clippy::single_match)]

// Limit this example to only compatible platforms.
#[cfg(any(windows_platform, macos_platform, x11_platform, wayland_platform,))]
fn main() -> Result<(), impl std::error::Error> {
    use simple_logger::SimpleLogger;

    use winit::{
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        platform::run_ondemand::EventLoopExtRunOnDemand,
        window::{Window, WindowBuilder},
    };

    #[path = "util/fill.rs"]
    mod fill;

    #[derive(Default)]
    struct App {
        window: Option<Window>,
    }

    SimpleLogger::new().init().unwrap();
    let mut event_loop = EventLoop::new();

    {
        let mut app = App::default();

        event_loop.run_ondemand(move |event, event_loop, control_flow| {
            control_flow.set_wait();
            println!("Run 1: {:?}", event);

            if let Some(window) = &app.window {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        window_id,
                    } if window.id() == window_id => {
                        app.window = None;
                        control_flow.set_exit();
                    }
                    Event::MainEventsCleared => window.request_redraw(),
                    Event::RedrawRequested(_) => {
                        fill::fill_window(window);
                    }
                    _ => (),
                }
            } else if let Event::Resumed = event {
                app.window = Some(
                    WindowBuilder::new()
                        .with_title("Fantastic window number one!")
                        .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
                        .build(event_loop)
                        .unwrap(),
                );
            }
        })?;
    }

    println!("--------------------------------------------------------- Finished first loop");
    println!("--------------------------------------------------------- Waiting 5 seconds");
    std::thread::sleep_ms(5000);

    let ret = {
        let mut app = App::default();

        event_loop.run_ondemand(move |event, event_loop, control_flow| {
            control_flow.set_wait();
            println!("Run 2: {:?}", event);

            if let Some(window) = &app.window {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        window_id,
                    } if window.id() == window_id => {
                        app.window = None;
                        control_flow.set_exit();
                    }
                    Event::MainEventsCleared => window.request_redraw(),
                    Event::RedrawRequested(_) => {
                        fill::fill_window(window);
                    }
                    _ => (),
                }
            } else if let Event::Resumed = event {
                app.window = Some(
                    WindowBuilder::new()
                        .with_title("Fantastic window number two!")
                        .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
                        .build(event_loop)
                        .unwrap(),
                );
            }
        })
    };

    println!("--------------------------------------------------------- Finished second loop");
    ret
}

#[cfg(not(any(windows_platform, macos_platform, x11_platform, wayland_platform,)))]
fn main() {
    println!("This example is not supported on this platform");
}
