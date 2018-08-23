extern crate winit;

use std::collections::HashMap;
use winit::window::Window;
use winit::event::{Event, WindowEvent, ElementState, KeyboardInput};
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let events_loop = EventLoop::new();

    let mut windows = HashMap::new();
    for _ in 0..3 {
        let window = Window::new(&events_loop).unwrap();
        windows.insert(window.id(), window);
    }

    events_loop.run(move |event, events_loop, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event, window_id } => {
                match event {
                    WindowEvent::CloseRequested => {
                        println!("Window {:?} has received the signal to close", window_id);

                        // This drops the window, causing it to close.
                        windows.remove(&window_id);

                        if windows.is_empty() {
                            *control_flow = ControlFlow::Exit;
                        }
                    },
                    WindowEvent::KeyboardInput { input: KeyboardInput { state: ElementState::Pressed, .. }, .. } => {
                        let window = Window::new(&events_loop).unwrap();
                        windows.insert(window.id(), window);
                    },
                    _ => ()
                }
            }
            _ => (),
        }
    })
}
