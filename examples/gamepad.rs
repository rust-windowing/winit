extern crate winit;
use winit::window::WindowBuilder;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event::device::GamepadEvent;
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("The world's worst video game")
        .build(&event_loop)
        .unwrap();

    let mut rumble_left = true;

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::GamepadEvent(gamepad_handle, event) => match event {
                GamepadEvent::Axis {..} => {
                    println!("[{:?}] {:#?}", gamepad_handle, event);
                },
                GamepadEvent::Button { state, .. } => {
                    println!("[{:?}] {:#?}", gamepad_handle, event);
                    match state {
                        ElementState::Pressed if rumble_left => gamepad_handle.rumble(1.0, 0.0),
                        ElementState::Pressed                => gamepad_handle.rumble(0.0, 1.0),
                        ElementState::Released => {
                            gamepad_handle.rumble(0.0, 0.0);
                            rumble_left = !rumble_left;
                        },
                    }
                },
                _ => ()
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => ()
        }
    });
}
