extern crate winit;

fn main() {
    let events_loop = winit::EventLoop::new();

    let window = winit::WindowBuilder::new().with_decorations(false)
                                                 .with_transparency(true)
                                                 .build(&events_loop).unwrap();

    window.set_title("A fantastic window!");

    events_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } =>
                *control_flow = winit::ControlFlow::Exit,
            _ => *control_flow = winit::ControlFlow::Wait,
        }
    });
}
