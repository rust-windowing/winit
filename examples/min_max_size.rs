extern crate winit;

mod helpers;

use winit::dpi::LogicalSize;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .build(&events_loop)
        .unwrap();

    // Wayland requires the commiting of a surface to display a window
    helpers::init_wayland(&window);

    window.set_min_dimensions(Some(LogicalSize::new(400.0, 200.0)));
    window.set_max_dimensions(Some(LogicalSize::new(800.0, 400.0)));

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } => winit::ControlFlow::Break,
            _ => winit::ControlFlow::Continue,
        }
    });
}
