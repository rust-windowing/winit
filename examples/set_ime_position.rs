use simple_logger::SimpleLogger;
use winit::{
    dpi::{LogicalPosition, Position},
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new().build(&event_loop).unwrap();
    window.set_title("A fantastic window!");

    println!("Ime position will system default");
    let mut ime_follow_cursor = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state: ElementState::Released,
                        ..
                    },
                ..
            } => {
                ime_follow_cursor = !ime_follow_cursor;
                if !ime_follow_cursor {
                    println!("Setting ime position to 10.0, 10.0");
                    window.set_ime_position(Position::Logical(LogicalPosition::new(10.0, 10.0)));
                } else {
                    println!("Ime will follow your mouse cursor");
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                if ime_follow_cursor {
                    println!("Setting ime position to {}, {}", position.x, position.y);
                    window.set_ime_position(position);
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
                return;
            }
            _ => (),
        }
    });
}
