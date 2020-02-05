mod utils;

use wasm_bindgen::prelude::*;
use winit::{
    event::{device::GamepadEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

/**
 * Build example (from examples/web):
 *    wasm-pack build --target web -- --features ${EXAMPLE}
 * Run web server (from examples/web):
 *    npx http-server
 *    Open your browser at http://localhost:8000/files/${EXAMPLE}.html
 * Development (from project root):
 *    npx nodemon --watch src --watch examples/web/src -e rs --exec 'cd examples/web && wasm-pack build --target web -- --features gamepad'
 */

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
  ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[cfg(feature = "gamepad")]
#[wasm_bindgen(start)]
pub fn example_gamepad() {
    utils::set_panic_hook(); // needed for error stack trace
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("Gamepad tests")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::GamepadEvent(gamepad_handle, event) => {
                match event {
                    // GamepadEvent::Axis {
                    //     axis_id,
                    //     axis,
                    //     value,
                    //     stick,
                    // } => console_log!("Axis {:#?} {:#?} {:#?} {:#?}", axis_id, axis, value, stick),

                    // // Discard any Stick event that falls inside the stick's deadzone.
                    // GamepadEvent::Stick {
                    //     x_value, y_value, ..
                    // } if (x_value.powi(2) + y_value.powi(2)).sqrt() < deadzone => (),
                    GamepadEvent::Button {
                        button_id,
                        button,
                        state,
                    } => console_log!("Button {:#?} {:#?} {:#?}", button_id, button, state),

                    GamepadEvent::Added => console_log!("[{:?}] {:#?}", gamepad_handle, event),
                    GamepadEvent::Removed => console_log!("[{:?}] {:#?}", gamepad_handle, event),
                    _ => {},
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
