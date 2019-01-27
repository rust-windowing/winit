extern crate winit;

mod helpers;

use std::collections::HashMap;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let mut windows = HashMap::new();
    for _ in 0..3 {
        let window = winit::Window::new(&events_loop).unwrap();
        // Wayland requires the commiting of a surface to display a window
        helpers::init_wayland(&window);
        windows.insert(window.id(), window);
    }

    events_loop.run_forever(|event| {
        match event {
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                window_id,
            } => {
                println!("Window {:?} has received the signal to close", window_id);

                // This drops the window, causing it to close.
                windows.remove(&window_id);

                if windows.is_empty() {
                    return winit::ControlFlow::Break;
                }
            }
            _ => (),
        }
        winit::ControlFlow::Continue
    })
}
