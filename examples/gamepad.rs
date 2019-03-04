extern crate winit;
use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use winit::event::device::GamepadEvent;
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("The world's worst video game")
        .build(&event_loop)
        .unwrap();

    println!("enumerating gamepads:");
    for gamepad in winit::event::device::GamepadHandle::enumerate(&event_loop) {
        println!("    gamepad {:?}", gamepad);
    }

    let deadzone = 0.12;

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::GamepadEvent(gamepad_handle, event) => match event {
                GamepadEvent::Axis{stick: true, ..} => (),
                GamepadEvent::Stick{x_value, y_value, ..} if (x_value.powi(2) + y_value.powi(2)).sqrt() < deadzone => (),
                _ => println!("[{:?}] {:#?}", gamepad_handle, event)
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => ()
        }
    });
}
