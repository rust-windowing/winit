extern crate winit;

use std::collections::HashMap;

fn main() {
    let mut events_loop = winit::EventLoop::new();

    let mut windows = HashMap::new();
    for _ in 0..3 {
        let window = winit::Window::new(&events_loop).unwrap();
        windows.insert(window.id(), window);
    }

    events_loop.run_forever(move |event, events_loop: &winit::EventLoop| {
        match event {
            winit::Event::WindowEvent { event, window_id } => {
                match event {
                    winit::WindowEvent::CloseRequested => {
                        println!("Window {:?} has received the signal to close", window_id);

                        // This drops the window, causing it to close.
                        windows.remove(&window_id);

                        if windows.is_empty() {
                            return winit::ControlFlow::Break;
                        }
                    },
                    winit::WindowEvent::KeyboardInput{..} => {
                        let window = winit::Window::new(&events_loop).unwrap();
                        windows.insert(window.id(), window);
                    },
                    _ => ()
                }
            }
            _ => (),
        }
        winit::ControlFlow::Continue
    })
}
