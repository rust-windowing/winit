mod utils;

extern crate web_sys;

extern crate winit;
use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};

use wasm_bindgen::prelude::*;

// A macro to provide `println!(..)`-style syntax for `console.log` logging.
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

// use wee_alloc if it is available
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
struct App {}

#[wasm_bindgen]
impl App {
    pub fn new() -> App {
        App{}
    }

    pub fn run(&self) {
        // create an event loop
        let event_loop = EventLoop::new();

        // create a window and associate it with the event loop
        let _window = WindowBuilder::new()
            .with_title("A fantastic window!")
            .build(&event_loop)
            .unwrap();

        // run!
        //
        // when using wasm_bindgen, this will currently throw a js
        // exception once all browser event handlers have been installed.
        event_loop.run(|event, _, control_flow| {
            log!("{:?}", event);

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                _ => *control_flow = ControlFlow::Poll,
            }
        });
    }
}
