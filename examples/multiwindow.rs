#![allow(clippy::single_match)]

use std::collections::HashMap;

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    window::Window,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let mut windows = HashMap::new();
    for _ in 0..3 {
        let window = Window::new(&event_loop).unwrap();
        println!("Opened a new window: {:?}", window.id());
        windows.insert(window.id(), window);
    }

    println!("Press N to open a new window.");

    event_loop.run(move |event, event_loop, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event, window_id } => {
                match event {
                    WindowEvent::CloseRequested => {
                        println!("Window {window_id:?} has received the signal to close");

                        // This drops the window, causing it to close.
                        windows.remove(&window_id);

                        if windows.is_empty() {
                            control_flow.set_exit();
                        }
                    }
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::N),
                                ..
                            },
                        is_synthetic: false,
                        ..
                    } => {
                        let window = Window::new(event_loop).unwrap();
                        println!("Opened a new window: {:?}", window.id());
                        windows.insert(window.id(), window);
                    }
                    _ => (),
                }
            }
            _ => (),
        }
    })
}
