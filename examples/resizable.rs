extern crate winit;
use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent, VirtualKeyCode, ElementState, KeyboardInput};
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let event_loop = EventLoop::new();

    let mut resizable = false;

    let window = WindowBuilder::new()
        .with_title("Hit space to toggle resizability.")
        .with_dimensions((400, 200).into())
        .with_resizable(resizable)
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Space),
                            state: ElementState::Released,
                            ..
                        },
                    ..
                } => {
                    resizable = !resizable;
                    println!("Resizable: {}", resizable);
                    window.set_resizable(resizable);
                }
                _ => (),
            },
            _ => (),
        };
    });
}
