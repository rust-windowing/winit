use simple_logger::SimpleLogger;
use winit::{
    dpi::PhysicalPosition,
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
    println!("Click to set ime position to cursor's");

    let mut cursor_position = PhysicalPosition::new(0.0, 0.0);
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                cursor_position = position;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state: ElementState::Released,
                        ..
                    },
                ..
            } => {
                println!(
                    "Setting ime position to {}, {}",
                    cursor_position.x, cursor_position.y
                );
                window.set_ime_position(cursor_position);
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
