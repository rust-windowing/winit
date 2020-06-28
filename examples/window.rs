use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        // println!("{:?}", event);

        match event {
            Event::WindowEvent(window_id, WindowEvent::CloseRequested)
                if window_id == window.id() =>
            {
                *control_flow = ControlFlow::Exit
            }
            Event::WindowEvent(_, e) => match e {
                WindowEvent::PointerCreated(..) |
                WindowEvent::PointerForce(..) |
                WindowEvent::PointerTilt(..) |
                WindowEvent::PointerTwist(..) |
                WindowEvent::PointerContactArea(..) |
                WindowEvent::PointerMoved(..) |
                WindowEvent::PointerButton(..) |
                WindowEvent::PointerEntered(..) |
                WindowEvent::PointerLeft(..) |
                WindowEvent::ScrollStarted |
                WindowEvent::ScrollLines(..) |
                WindowEvent::ScrollPixels(..) |
                WindowEvent::ScrollEnded |
                WindowEvent::PointerDestroyed(..) => println!("{:?}", e),
                _ => ()
            },
            Event::MainEventsCleared => {
                window.request_redraw()
            },
            _ => (),
        }
    });
}
