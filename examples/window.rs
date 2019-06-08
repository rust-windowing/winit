extern crate winit;
#[cfg(feature = "stdweb")]
#[macro_use]
extern crate stdweb;
#[cfg(feature = "wasm-bindgen")]
extern crate wasm_bindgen;
#[cfg(feature = "wasm-bindgen")]
extern crate web_sys;

use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::console;

#[wasm_bindgen(start)]
pub fn main() {
    console::log_1(&JsValue::from_str("main"));
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();
    console::log_1(&JsValue::from_str("Created window"));

    event_loop.run(|event, _, control_flow| {
        console::log_1(&JsValue::from_str(&format!("{:?}", event)));

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => *control_flow = ControlFlow::Wait,
        }
    });
}