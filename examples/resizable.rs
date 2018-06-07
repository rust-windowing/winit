extern crate winit;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("Hit space to toggle resizability.")
        .with_dimensions(400, 200)
        .with_resizable(false)
        .build(&events_loop)
        .unwrap();

    let mut resizable = false;

    events_loop.run_forever(|event| match event {
        winit::Event::WindowEvent { event, .. } => match event {
            winit::WindowEvent::CloseRequested => winit::ControlFlow::Break,
            winit::WindowEvent::KeyboardInput {
                input:
                    winit::KeyboardInput {
                        virtual_keycode: Some(winit::VirtualKeyCode::Space),
                        state: winit::ElementState::Released,
                        ..
                    },
                ..
            } => {
                resizable = !resizable;
                println!("Resizable: {}", resizable);
                window.set_resizable(resizable);
                winit::ControlFlow::Continue
            }
            _ => winit::ControlFlow::Continue,
        },
        _ => winit::ControlFlow::Continue,
    });
}
