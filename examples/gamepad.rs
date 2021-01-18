use winit::event::device::{GamepadEvent, GamepadHandle};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

fn main() {
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("The world's worst video game")
        .build(&event_loop)
        .unwrap();

    println!("enumerating gamepads:");
    for gamepad in GamepadHandle::enumerate(&event_loop) {
        println!(
            "    gamepad={:?}\tport={:?}\tbattery level={:?}",
            gamepad,
            gamepad.port(),
            gamepad.battery_level()
        );
    }

    let deadzone = 0.12;

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::GamepadEvent(gamepad_handle, event) => {
                match event {
                    // Discard any Axis events that has a corresponding Stick event.
                    GamepadEvent::Axis { stick: true, .. } => (),

                    // Discard any Stick event that falls inside the stick's deadzone.
                    GamepadEvent::Stick {
                        x_value, y_value, ..
                    } if (x_value.powi(2) + y_value.powi(2)).sqrt() < deadzone => (),

                    _ => println!("[{:?}] {:#?}", gamepad_handle, event),
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
