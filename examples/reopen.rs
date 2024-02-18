#![allow(clippy::single_match)]

use std::collections::HashMap;

use simple_logger::SimpleLogger;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{EventLoop, EventLoopWindowTarget},
    window::{Window, WindowId},
};

#[path = "util/fill.rs"]
mod fill;

fn new_window(event_loop: &EventLoopWindowTarget, windows: &mut HashMap<WindowId, Window>) {
    let window = Window::new(event_loop).unwrap();
    println!("Opened a new window: {:?}", window.id());
    windows.insert(window.id(), window);
}

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let mut windows = HashMap::new();
    new_window(&event_loop, &mut windows);

    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, window_id } = event {
            match event {
                WindowEvent::CloseRequested => {
                    println!("Window {window_id:?} has received the signal to close");

                    // This drops the window, causing it to close.
                    windows.remove(&window_id);

                    // Keep the event loop running on macOS, even if all windows are closed.
                    #[cfg(not(target_os = "macos"))]
                    if windows.is_empty() {
                        elwt.exit();
                    }
                }
                WindowEvent::RedrawRequested => {
                    if let Some(window) = windows.get(&window_id) {
                        fill::fill_window(window);
                    }
                }
                _ => (),
            }
        } else if let Event::Reopen {
            has_visible_windows,
        } = event
        {
            println!("Reopen event: has_visible_windows={}", has_visible_windows);
            // If there are no visible windows, open a new one.
            if !has_visible_windows {
                new_window(elwt, &mut windows);
            }
        }
    })
}
