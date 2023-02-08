use simple_logger::SimpleLogger;
use winit::{event::Event, event_loop::EventLoop, window::WindowBuilder};

fn main() {
    SimpleLogger::new().init().unwrap();

    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("An iconic window!")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        if let Event::WindowEvent { event, .. } = event {
            use winit::event::WindowEvent::*;
            match event {
                CloseRequested => control_flow.set_exit(),
                DragEnter { .. } | DragOver { .. } | DragDrop { .. } | DragLeave => {
                    println!("{:?}", event);
                }
                _ => (),
            }
        }
    });
}
