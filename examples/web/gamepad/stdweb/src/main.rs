use winit::{
    event::{device::GamepadEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use stdweb::js;

/**
 * Build example (from examples/web/gamepad/stdweb):
 *    cargo web build
 * Run example (from examples/web/gamepad/stdweb):
 *    cargo web start
 * Development (from project root):
 *    npx nodemon --watch src --watch examples/web/gamepad/stdweb/src -e rs --exec 'cargo web check'
 */

pub fn main() {
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("Gamepad tests")
        .build(&event_loop)
        .unwrap();

    let deadzone = 0.12;

    event_loop.run(move |event, _, control_flow| match event {
        Event::GamepadEvent(gamepad_handle, event) => match event {
            GamepadEvent::Axis {
                axis_id,
                axis,
                value,
                stick,
            } if value > deadzone => {
                let string = format!("Axis {:#?} {:#?} {:#?} {:#?}", axis_id, axis, value, stick);
                js! { console.log( @{string} ); }
            }

            GamepadEvent::Stick {
                x_id,
                y_id,
                x_value,
                y_value,
                side,
            } if (x_value.powi(2) + y_value.powi(2)).sqrt() > deadzone => {
                let string = format!(
                    "Stick {:#?} {:#?} {:#?} {:#?} {:#?}",
                    x_id, y_id, x_value, y_value, side
                );
                js! { console.log( @{string} ); }
            }

            GamepadEvent::Button {
                button_id,
                button,
                state,
            } => {
                let string = format!("Button {:#?} {:#?} {:#?}", button_id, button, state);
                js! { console.log( @{string} ); }
            }

            GamepadEvent::Added => {
                let string = format!("[{:?}] {:#?}", gamepad_handle, event);
                js! { console.log( @{string} ); }
            }
            GamepadEvent::Removed => {
                let string = format!("[{:?}] {:#?}", gamepad_handle, event);
                js! { console.log( @{string} ); }
            }

            _ => {}
        },
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *control_flow = ControlFlow::Exit,
        _ => (),
    });
}
