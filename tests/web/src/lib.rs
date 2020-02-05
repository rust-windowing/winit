mod utils;

use winit::{
    event::{
        device::GamepadEvent,
        Event, WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

// From tests/web: wasm-pack test --firefox --headless
// From tests/web: wasm-pack build --target web
// From (project root): npx nodemon --watch src --watch tests/web/src -e rs --exec 'cd tests/web && wasm-pack build --target web'

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
  // Note that this is using the `log` function imported above during
  // `bare_bones`
  ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

extern crate wasm_bindgen_test;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// #[wasm_bindgen_test]
#[wasm_bindgen]
pub fn test_gamepad() {
    utils::set_panic_hook();
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("Gamepad tests")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::GamepadEvent(gamepad_handle, event) => {
                match event {
                    GamepadEvent::Axis {
                        axis_id,
                        axis,
                        value,
                        stick,
                    } => console_log!("Axis {:#?} {:#?} {:#?} {:#?}", axis_id, axis, value, stick),

                    // // Discard any Stick event that falls inside the stick's deadzone.
                    // GamepadEvent::Stick {
                    //     x_value, y_value, ..
                    // } if (x_value.powi(2) + y_value.powi(2)).sqrt() < deadzone => (),

                    GamepadEvent::Button {
                        button_id,
                        button,
                        state
                    } => console_log!("Button {:#?} {:#?} {:#?}", button_id, button, state),

                    _ => console_log!("[{:?}] {:#?}", gamepad_handle, event),
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
