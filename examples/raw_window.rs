extern crate winit;

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

    let evlp_raw = unsafe {
        winit::EventsLoop::new_from_raw_parts(&evlp_raw)
    };

    let _win_raw = unsafe {
        winit::Window::new_from_raw_parts(&evlp_raw, &win_raw).unwrap()
    };

    let mut running = true;
    while running {
        evlp.poll_events(|event| {
            println!("Evlp {:?}", event);
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
                    | winit::WindowEvent::CloseRequested => running = false,
                    _ => (),
                }
            }
        });
    }
}
