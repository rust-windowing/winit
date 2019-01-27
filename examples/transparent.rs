extern crate winit;

mod helpers;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new().with_decorations(false)
                                                 .with_transparency(true)
                                                 .build(&events_loop).unwrap();

    // Wayland requires the commiting of a surface to display a window
    helpers::init_wayland(&window);

    window.set_title("A fantastic window!");

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } => winit::ControlFlow::Break,
            _ => winit::ControlFlow::Continue,
        }
    });
}
