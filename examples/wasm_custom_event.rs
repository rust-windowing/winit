#![allow(clippy::single_match)]

use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{EventLoop, EventLoopBuilder, EventLoopProxy},
    window::WindowBuilder,
};

#[derive(Debug, Clone, Copy)]
pub enum CustomEvent {
    Add { a: u32, b: u32 },
}

thread_local! {
    pub static EVENT_LOOP_PROXY: RefCell<Option<EventLoopProxy<CustomEvent>>> = RefCell::new(None);
}

fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = "fireAdd"))]
pub fn fire_add() {
    EVENT_LOOP_PROXY.with(|proxy| {
        if let Some(event_loop_proxy) = proxy.borrow().as_ref() {
            event_loop_proxy
                .send_event(CustomEvent::Add { a: 1, b: 2 })
                .ok();
        }
    });
}

pub fn main() {
    let event_loop: EventLoop<CustomEvent> =
        EventLoopBuilder::<CustomEvent>::with_user_event().build();

    let event_loop_proxy = event_loop.create_proxy();

    EVENT_LOOP_PROXY.with(move |proxy| {
        proxy.replace(Some(event_loop_proxy));
    });

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    #[cfg(wasm_platform)]
    wasm::insert_canvas(&window);

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => control_flow.set_exit(),
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::UserEvent(CustomEvent::Add { a, b }) => {
                let result = add(a, b);
                log::info!("{:?}", result);
            }
            _ => (),
        }
    });
}

#[cfg(wasm_platform)]
mod wasm {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::HtmlScriptElement;
    use winit::{event::Event, window::Window};

    #[wasm_bindgen(start)]
    pub fn run() {
        console_log::init_with_level(log::Level::Debug).expect("error initializing logger");

        #[allow(clippy::main_recursion)]
        super::main();
    }

    pub fn insert_canvas(window: &Window) {
        use winit::platform::web::WindowExtWebSys;

        let canvas = window.canvas();

        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let body = document.body().unwrap();

        // Set a background color for the canvas to make it easier to tell where the canvas is for debugging purposes.
        canvas.style().set_css_text("background-color: crimson;");
        body.append_child(&canvas).unwrap();

        // Create script element
        let script: HtmlScriptElement = document
            .create_element("script")
            .unwrap()
            .dyn_into()
            .unwrap();
        script.set_type("module");

        // Your JavaScript code here, including creating the button and attaching the event handler
        script.set_inner_text(
            r#"
console.log("Custom Button Loaded");
import { fireAdd } from "./wasm_custom_event.js";
fireAdd();
let button = document.createElement("button");
button.innerHTML = "Click me!";
button.onclick = () => {
    console.log("Button Clicked", fireAdd);
    fireAdd();
};
document.body.appendChild(button);
console.log("Custom Button Loaded 2");

            "#,
        );

        let first_child = body.first_child();
        match first_child {
            Some(node) => body.insert_before(&script, Some(&node)).unwrap(),
            None => body.append_child(&script).unwrap(),
        };
    }
}
