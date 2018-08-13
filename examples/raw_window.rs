extern crate winit;

use std::sync::Mutex;

fn main() {
    let mut evlp = winit::EventsLoop::new();
    let win = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&evlp)
        .unwrap();

    let evlp_raw = evlp.get_raw_parts();
    let win_raw = win.get_raw_parts();

    // Replace the code above with some *other* magical way to make your raw parts.
    // I.e. some C library.

    let mut evlp_raw = unsafe {
        winit::EventsLoop::new_from_raw_parts(&evlp_raw)
    };

    let _win_raw = unsafe {
        winit::Window::new_from_raw_parts(&evlp_raw, &win_raw).unwrap()
    };

    let running = Mutex::new(true);
    while *running.lock().unwrap() {
        let event_handler = |event| {
            if let winit::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::WindowEvent::KeyboardInput {
                        input:
                            winit::KeyboardInput {
                                virtual_keycode: Some(winit::VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    }
                    | winit::WindowEvent::CloseRequested => *running.lock().unwrap() = false,
                    _ => (),
                }
            }
        };

        evlp.poll_events(|event| {
            println!("Evlp {:?}", event);
            event_handler(event)
        });
        evlp_raw.poll_events(|event| {
            println!("Raw Evlp {:?}", event);
            event_handler(event)
        });
    }
}
