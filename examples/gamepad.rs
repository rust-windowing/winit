extern crate winit;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("The world's worst video game")
        .build(&events_loop)
        .unwrap();

    events_loop.run_forever(|event| {
        match event {
            winit::Event::DeviceEvent { device_id, event } => match event {
                winit::DeviceEvent::Button { button, state } => {
                    println!("[{:?}] {:#?}", device_id, event);
                    winit::ControlFlow::Continue
                },
                winit::DeviceEvent::Motion { axis, value } => {
                    println!("[{:?}] {:#?}", device_id, event);
                    winit::ControlFlow::Continue
                },
                _ => winit::ControlFlow::Continue,
            },
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                ..
            } => winit::ControlFlow::Break,
            _ => winit::ControlFlow::Continue,
        }
    });
}
