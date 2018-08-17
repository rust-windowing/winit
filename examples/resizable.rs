extern crate winit;

fn main() {
    let events_loop = winit::EventLoop::new();

    let mut resizable = false;

    let window = winit::WindowBuilder::new()
        .with_title("Hit space to toggle resizability.")
        .with_dimensions((400, 200).into())
        .with_resizable(resizable)
        .build(&events_loop)
        .unwrap();

    events_loop.run(move |event, _, control_flow| {
        *control_flow = winit::ControlFlow::Wait;
        match event {
            winit::Event::WindowEvent { event, .. } => match event {
                winit::WindowEvent::CloseRequested => *control_flow = winit::ControlFlow::Exit,
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
                }
                _ => (),
            },
            _ => (),
        };
    });
}
