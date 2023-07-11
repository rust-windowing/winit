#![allow(clippy::single_match)]

#[cfg(not(wasm_platform))]
fn main() {
    use std::{sync::Arc, thread, time};

    use simple_logger::SimpleLogger;
    use winit::{
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        window::WindowBuilder,
    };

    #[path = "util/fill.rs"]
    mod fill;

    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = {
        let window = WindowBuilder::new()
            .with_title("A fantastic window!")
            .build(&event_loop)
            .unwrap();
        Arc::new(window)
    };

    thread::spawn({
        let window = window.clone();
        move || loop {
            thread::sleep(time::Duration::from_secs(1));
            window.request_redraw();
        }
    });

    event_loop.run(move |event, _, control_flow| {
        println!("{event:?}");

        control_flow.set_wait();

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => control_flow.set_exit(),
            Event::RedrawRequested(_) => {
                println!("\nredrawing!\n");
                fill::fill_window(&window);
            }
            _ => (),
        }
    });
}

#[cfg(wasm_platform)]
fn main() {
    unimplemented!() // `Window` can't be sent between threads
}
