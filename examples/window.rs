extern crate winit;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    for monitor in events_loop.get_available_monitors() {
        println!("the extent of the monitor is: {:?}", monitor.get_physical_extents());
    }

    events_loop.run_forever(|event| {
        // println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } => {
                winit::ControlFlow::Break
            },
            _ => winit::ControlFlow::Continue,
        }
    });
}
