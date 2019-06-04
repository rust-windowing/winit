extern crate winit;
#[cfg(feature = "stdweb")]
#[macro_use]
extern crate stdweb;
#[cfg(feature = "wasm-bindgen")]
#[macro_use]
extern crate stdweb;

use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();
    //console!(log, "Built window!");

    event_loop.run(|event, _, control_flow| {
        //console!(log, format!("{:?}", event));

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => *control_flow = ControlFlow::Wait,
        }
    });
}
