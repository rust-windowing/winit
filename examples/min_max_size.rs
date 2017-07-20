extern crate winit;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_min_dimensions(400, 200)
        .with_max_dimensions(800, 400)
        .build(&events_loop)
        .unwrap();

    if cfg!(target_os = "linux") {
        println!("Running this example under wayland may not display a window at all.\n\
                  This is normal and because this example does not actually draw anything in the window,\
                  thus the compositor does not display it.");
    }

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } => winit::ControlFlow::Break,
            _ => winit::ControlFlow::Continue,
        }
    });
}
