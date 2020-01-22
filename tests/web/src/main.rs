use winit::{
    event::{
        device::{GamepadEvent, GamepadHandle},
        Event, WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

// wasm-pack test --firefox --headless

// use wasm_bindgen::prelude::*;

// #[wasm_bindgen]
// extern "C" {
//     // Use `js_namespace` here to bind `console.log(..)` instead of just
//     // `log(..)`
//     #[wasm_bindgen(js_namespace = console)]
//     fn log(s: &str);

//     // The `console.log` is quite polymorphic, so we can bind it with multiple
//     // signatures. Note that we need to use `js_name` to ensure we always call
//     // `log` in JS.
//     #[wasm_bindgen(js_namespace = console, js_name = log)]
//     fn log_u32(a: u32);

//     // Multiple arguments too!
//     #[wasm_bindgen(js_namespace = console, js_name = log)]
//     fn log_many(a: &str, b: &str);
// }

// macro_rules! console_log {
//   // Note that this is using the `log` function imported above during
//   // `bare_bones`
//   ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
// }

extern crate wasm_bindgen_test;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_create_window() {
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            _ => *control_flow = ControlFlow::Wait,
        }
    });
}

// #[wasm_bindgen_test]
// fn test_gamepad() {
//     let event_loop = EventLoop::new();

//     let _window = WindowBuilder::new()
//         .with_title("The world's worst video game")
//         .build(&event_loop)
//         .unwrap();

//     event_loop.run(move |evemt, _, control_flow| {
//         match evemt {
//             Event::GamepadEvent(gamepad_handle, event) => {
//                 match event {
//                     // // Discard any Axis events that has a corresponding Stick event.
//                     // GamepadEvent::Axis { stick: true, .. } => (),

//                     // // Discard any Stick event that falls inside the stick's deadzone.
//                     // GamepadEvent::Stick {
//                     //     x_value, y_value, ..
//                     // } if (x_value.powi(2) + y_value.powi(2)).sqrt() < deadzone => (),
//                     _ => console_log!("[{:?}] {:#?}", gamepad_handle, event),
//                 }
//             }
//             Event::WindowEvent {
//                 event: WindowEvent::CloseRequested,
//                 ..
//             } => *control_flow = ControlFlow::Exit,
//             _ => (),
//         }
//     });
// }
