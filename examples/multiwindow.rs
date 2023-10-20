#![allow(clippy::single_match)]

use std::collections::HashMap;

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, NamedKey},
    window::Window,
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let mut windows = HashMap::new();
    for _ in 0..3 {
        let window = Window::new(&event_loop).unwrap();
        println!("Opened a new window: {:?}", window.id());
        windows.insert(window.id(), window);
    }

    println!("Press N to open a new window.");

    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, window_id } = event {
            match event {
                WindowEvent::CloseRequested => {
                    println!("Window {window_id:?} has received the signal to close");

                    // This drops the window, causing it to close.
                    windows.remove(&window_id);

                    if windows.is_empty() {
                        elwt.exit();
                    }
                }
                WindowEvent::KeyboardInput {
                    event,
                    is_synthetic: false,
                    ..
                } if event.state == ElementState::Pressed => match event.logical_key {
                    Key::Named(NamedKey::Escape) => elwt.exit(),
                    Key::Character(c) if c == "n" || c == "N" => {
                        let window = Window::new(elwt).unwrap();
                        println!("Opened a new window: {:?}", window.id());
                        windows.insert(window.id(), window);
                    }
                    _ => (),
                },
                WindowEvent::RedrawRequested => {
                    if let Some(window) = windows.get(&window_id) {
                        fill::fill_window(window);
                    }
                }
                _ => (),
            }
        }
    })
}
