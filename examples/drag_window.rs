use simple_logger::SimpleLogger;
use winit::{
    event::{
        ElementState, Event, KeyboardInput, MouseButton, StartCause, VirtualKeyCode, WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window_1 = WindowBuilder::new().build(&event_loop).unwrap();
    let window_2 = WindowBuilder::new().build(&event_loop).unwrap();

    let mut switched = false;

    event_loop.run(move |event, _, control_flow| match event {
        Event::NewEvents(StartCause::Init) => {
            eprintln!("Switch which window is to be dragged by pressing \"x\".")
        }
        Event::WindowEvent { event, window_id } => match event {
            WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let window = if (window_id == window_1.id() && switched)
                    || (window_id == window_2.id() && !switched)
                {
                    &window_2
                } else {
                    &window_1
                };

                window.set_drag_window().unwrap()
            }
            WindowEvent::CursorEntered { .. } => {
                let (drag_target, other) = if (window_id == window_1.id() && switched)
                    || (window_id == window_2.id() && !switched)
                {
                    (&window_2, &window_1)
                } else {
                    (&window_1, &window_2)
                };
                drag_target.set_title("drag target");
                other.set_title("winit window");
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Released,
                        virtual_keycode: Some(VirtualKeyCode::X),
                        ..
                    },
                ..
            } => {
                switched = !switched;
                println!("Switched!")
            }
            _ => (),
        },
        _ => (),
    });
}
