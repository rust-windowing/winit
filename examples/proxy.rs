extern crate winit;
use winit::{EventLoop, WindowBuilder};

fn main() {
    let events_loop: EventLoop<i32> = EventLoop::new_user_event();

    let _window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    let proxy = events_loop.create_proxy();

    std::thread::spawn(move || {
        let mut counter = 0;
        // Wake up the `events_loop` once every second.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            proxy.send_event(counter).unwrap();
            counter += 1;
        }
    });

    events_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);
        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } =>
                *control_flow = winit::ControlFlow::Exit,
            _ => *control_flow = winit::ControlFlow::Wait,
        }
    });
}
