#![allow(clippy::single_match)]

#[cfg(not(wasm_platform))]
fn main() {
    use simple_logger::SimpleLogger;
    use winit::{
        event::{Event, WindowEvent},
        event_loop::EventLoopBuilder,
        window::WindowBuilder,
    };

    #[path = "util/fill.rs"]
    mod fill;

    #[derive(Debug, Clone, Copy)]
    enum CustomEvent {
        Timer,
    }

    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoopBuilder::<CustomEvent>::with_user_event().build();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    // `EventLoopProxy` allows you to dispatch custom events to the main Winit event
    // loop from any thread.
    let event_loop_proxy = event_loop.create_proxy();

    std::thread::spawn(move || {
        // Wake up the `event_loop` once every second and dispatch a custom event
        // from a different thread.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            event_loop_proxy.send_event(CustomEvent::Timer).ok();
        }
    });

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::UserEvent(event) => println!("user event: {event:?}"),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => control_flow.set_exit(),
            Event::RedrawRequested(_) => {
                fill::fill_window(&window);
            }
            _ => (),
        }
    });
}

#[cfg(wasm_platform)]
fn main() {
    panic!("This example is not supported on web.");
}
