use {
    simple_logger::SimpleLogger,
    winit::{
        event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    },
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("Highly unstable window")
        .build(&event_loop)
        .unwrap();

    println!("Use your number keys (0-9) to enter an exit code!");

    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(keycode),
                                ..
                            },
                        ..
                    },
                ..
            } => match keycode {
                // actually perfect for a macro, but for simplicity I'll just refrain from that
                VirtualKeyCode::Key1 => *flow = ControlFlow::Exit(1),
                VirtualKeyCode::Key2 => *flow = ControlFlow::Exit(2),
                VirtualKeyCode::Key3 => *flow = ControlFlow::Exit(3),
                VirtualKeyCode::Key4 => *flow = ControlFlow::Exit(4),
                VirtualKeyCode::Key5 => *flow = ControlFlow::Exit(5),
                VirtualKeyCode::Key6 => *flow = ControlFlow::Exit(6),
                VirtualKeyCode::Key7 => *flow = ControlFlow::Exit(7),
                VirtualKeyCode::Key8 => *flow = ControlFlow::Exit(8),
                VirtualKeyCode::Key9 => *flow = ControlFlow::Exit(9),
                VirtualKeyCode::Key0 => *flow = ControlFlow::Exit(0),
                _ => (),
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *flow = ControlFlow::Exit(47),
            _ => (),
        }
    });
}
