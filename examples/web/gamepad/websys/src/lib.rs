mod utils;

use wasm_bindgen::prelude::*;
use winit::{
    event::{device::GamepadEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

/**
 * Build example (from examples/gamepad/websys):
 *    wasm-pack build --target web
 * Run web server (from examples/gamepad/websys):
 *    npx http-server
 *    Open your browser at http://localhost:8000/files/${EXAMPLE}.html
 * Development (from project root):
 *    npx nodemon --watch src --watch examples/web/gamepad/websys/src -e rs --exec 'cd examples/web/gamepad/websys && wasm-pack build --target web'
 */

macro_rules! console_log {
  ($($t:tt)*) => (web_sys::console::log_1(&format_args!($($t)*).to_string().into()))
}

#[wasm_bindgen(start)]
pub fn example_gamepad() {
    utils::set_panic_hook(); // needed for error stack trace
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
                console_log!("Axis {:#?} {:#?} {:#?} {:#?}", axis_id, axis, value, stick)
            }

            GamepadEvent::Stick {
                x_id,
                y_id,
                x_value,
                y_value,
                side,
            } if (x_value.powi(2) + y_value.powi(2)).sqrt() > deadzone => {
                console_log!(
                    "Stick {:#?} {:#?} {:#?} {:#?} {:#?}",
                    x_id,
                    y_id,
                    x_value,
                    y_value,
                    side
                )
            }

            GamepadEvent::Button {
                button_id,
                button,
                state,
            } => {
                console_log!("Button {:#?} {:#?} {:#?}", button_id, button, state)
            }

            GamepadEvent::Added => {
                console_log!("[{:?}] {:#?}", gamepad_handle, event)
            }
            GamepadEvent::Removed => console_log!("[{:?}] {:#?}", gamepad_handle, event),

            _ => {}
        },
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *control_flow = ControlFlow::Exit,
        _ => (),
    });
}
