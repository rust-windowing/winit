#[cfg(all(target_arch = "wasm32", feature = "stdweb"))]
fn main() {
    use winit::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        platform::web::WindowExtStdweb,
        window::WindowBuilder,
    };
    // Note: stdweb is aliased to std_web in winit's Cargo.toml
    // In most cases, it should not be necessary to include this line
    use std_web as stdweb;
    use stdweb::{
        traits::*,
        web::document
    };

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    document().body().unwrap().append_child(&window.canvas());

    event_loop.run(move |event, _, control_flow| {
        stdweb::console!(log, format!("{:?}", event));

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            _ => *control_flow = ControlFlow::Wait,
        }
    });
}

#[cfg(not(all(target_arch = "wasm32", feature = "stdweb")))]
fn main() {}
