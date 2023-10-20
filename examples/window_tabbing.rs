#![allow(clippy::single_match)]

#[cfg(target_os = "macos")]
use std::{collections::HashMap, num::NonZeroUsize};

#[cfg(target_os = "macos")]
use simple_logger::SimpleLogger;
#[cfg(target_os = "macos")]
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, NamedKey},
    platform::macos::{WindowBuilderExtMacOS, WindowExtMacOS},
    window::{Window, WindowBuilder},
};

#[cfg(target_os = "macos")]
#[path = "util/fill.rs"]
mod fill;

#[cfg(target_os = "macos")]
fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let mut windows = HashMap::new();
    let window = Window::new(&event_loop).unwrap();
    println!("Opened a new window: {:?}", window.id());
    windows.insert(window.id(), window);

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
                WindowEvent::Resized(_) => {
                    if let Some(window) = windows.get(&window_id) {
                        window.request_redraw();
                    }
                }
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            state: ElementState::Pressed,
                            logical_key,
                            ..
                        },
                    is_synthetic: false,
                    ..
                } => match logical_key.as_ref() {
                    Key::Character("t") => {
                        let tabbing_id = windows.get(&window_id).unwrap().tabbing_identifier();
                        let window = WindowBuilder::new()
                            .with_tabbing_identifier(&tabbing_id)
                            .build(elwt)
                            .unwrap();
                        println!("Added a new tab: {:?}", window.id());
                        windows.insert(window.id(), window);
                    }
                    Key::Character("w") => {
                        let _ = windows.remove(&window_id);
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        windows.get(&window_id).unwrap().select_next_tab();
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        windows.get(&window_id).unwrap().select_previous_tab();
                    }
                    Key::Character(ch) => {
                        if let Ok(index) = ch.parse::<NonZeroUsize>() {
                            let index = index.get();
                            // Select the last tab when pressing `9`.
                            let window = windows.get(&window_id).unwrap();
                            if index == 9 {
                                window.select_tab_at_index(window.num_tabs() - 1)
                            } else {
                                window.select_tab_at_index(index - 1);
                            }
                        }
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

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("This example is only supported on MacOS");
}
