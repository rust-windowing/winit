extern crate winit;
use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let event_loop: EventLoop<i32> = EventLoop::new_user_event();

    let _window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    let proxy = event_loop.create_proxy();

    std::thread::spawn(move || {
        let mut counter = 0;
        // Wake up the `event_loop` once every second.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            proxy.send_event(counter).unwrap();
            counter += 1;
        }
    });

    event_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } =>
                *control_flow = ControlFlow::Exit,
            _ => *control_flow = ControlFlow::Wait,
        }
    });
}
