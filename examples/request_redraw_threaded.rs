#![allow(clippy::single_match)]

#[cfg(not(wasm_platform))]
fn main() {
    use std::{thread, time};

    use simple_logger::SimpleLogger;
    use winit::{
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        window::WindowBuilder,
    };

    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    thread::spawn(move || loop {
        thread::sleep(time::Duration::from_secs(1));
        window.request_redraw();
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
            }
            _ => (),
        }
    });
}

#[cfg(wasm_platform)]
fn main() {
    unimplemented!() // `Window` can't be sent between threads
}
