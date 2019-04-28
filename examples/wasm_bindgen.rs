extern crate web_sys;
extern crate wasm_bindgen;

extern crate winit;
use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use winit::platform::websys::WebsysWindowExt;
use winit::platform::websys::WebsysWindowBuilderExt;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

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
        let window = WindowBuilder::new()
            .with_title("A fantastic window!")
            .with_canvas_id("test")
            .build(&event_loop)
            .unwrap();

        // do some drawing
        let canvas = window.get_canvas();
        let ctx = canvas.get_context("2d").unwrap().unwrap()
                        .dyn_into::<web_sys::CanvasRenderingContext2d>().unwrap();
        ctx.begin_path();
        ctx.arc(95.0, 50.0, 40.0, 0.0, 2.0 * 3.14159).unwrap();
        ctx.stroke();

        // run forever
        //
        // when using wasm_bindgen, this will currently throw a js
        // exception once all browser event handlers have been installed.
        event_loop.run(|event, _, control_flow| {

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

pub fn main() {
    let app = App::new();
    app.run();
}