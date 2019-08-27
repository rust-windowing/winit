use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        // .with_resizable(false)
        .with_inner_size(winit::dpi::PhysicalSize::new(256, 256))
        .build(&event_loop)
        .unwrap();

    let assert_size = window.inner_size();

    event_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        // assert_eq!(assert_size, window.inner_size());

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            // Event::WindowEvent {
            //     event: WindowEvent::HiDpiFactorChanged{new_inner_size, ..},
            //     ..
            // } => {
            //     *new_inner_size = Some(assert_size);
            // }
            _ => *control_flow = ControlFlow::Wait,
        }
    });
}
