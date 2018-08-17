extern crate winit;

fn main() {
    let events_loop = winit::EventLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    let proxy = events_loop.create_proxy();

    std::thread::spawn(move || {
        // Wake up the `events_loop` once every second.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            proxy.wakeup().unwrap();
        }
    });

    events_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);
        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } =>
                *control_flow = winit::ControlFlow::Wait,
            _ => *control_flow = winit::ControlFlow::Wait,
        }
    });
}
