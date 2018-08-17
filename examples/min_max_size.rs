extern crate winit;

use winit::dpi::LogicalSize;

fn main() {
    let events_loop = winit::EventLoop::new();

    let window = winit::WindowBuilder::new()
        .build(&events_loop)
        .unwrap();

    window.set_min_dimensions(Some(LogicalSize::new(400.0, 200.0)));
    window.set_max_dimensions(Some(LogicalSize::new(800.0, 400.0)));

    events_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } =>
                *control_flow = winit::ControlFlow::Exit,
            _ => *control_flow = winit::ControlFlow::Wait,
        }
    });
}
