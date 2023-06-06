/// This example will show how to call a Rust function from JavaScript
/// You should see the number 42 appear in browser console output when you click the button

#[cfg(not(wasm_platform))]
pub fn main() {
    panic!("This example is only meant to be compiled for wasm target")
}

#[cfg(wasm_platform)]
pub fn main() {
    panic!("Please run `cargo run-wasm --example wasm_custom_event`")
}

#[cfg(wasm_platform)]
mod wasm {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::HtmlScriptElement;
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::{EventLoop, EventLoopBuilder, EventLoopProxy};
    use winit::window::{Window, WindowBuilder};

    // Because EventLoopProxy is not Send, we need to wrap it in a RefCell and use thread_local!
    thread_local! {
        pub static EVENT_LOOP_PROXY: RefCell<Option<EventLoopProxy<CustomEvent>>> = RefCell::new(None);
    }

    // Function to be called from JS
    fn wasm_call() -> u32 {
        42
    }

    #[derive(Debug, Clone, Copy)]
    pub enum CustomEvent {
        WasmCall,
    }

    #[wasm_bindgen(start)]
    pub fn run() {
        console_log::init_with_level(log::Level::Debug).expect("error initializing logger");

        let event_loop: EventLoop<CustomEvent> =
            EventLoopBuilder::<CustomEvent>::with_user_event().build();

        let event_loop_proxy = event_loop.create_proxy();

        // Initialize the thread_local EVENT_LOOP_PROXY value
        EVENT_LOOP_PROXY.with(move |proxy| {
            proxy.replace(Some(event_loop_proxy));
        });

        let window = WindowBuilder::new().build(&event_loop).unwrap();

        insert_canvas(&window);

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
                // Handle custom events here
                Event::UserEvent(CustomEvent::WasmCall) => {
                    // Send the result back to JS as proof that the custom event was handled
                    log::info!("{:?}", wasm_call());
                }
                _ => (),
            }
        });
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = "handleWasmCall"))]
    pub fn handle_wasm_call() {
        EVENT_LOOP_PROXY.with(|proxy| {
            if let Some(event_loop_proxy) = proxy.borrow().as_ref() {
                event_loop_proxy.send_event(CustomEvent::WasmCall).ok();
            }
        });
    }

    pub fn insert_canvas(window: &Window) {
        use winit::platform::web::WindowExtWebSys;

        let canvas = window.canvas();

        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let body = document.body().unwrap();

        // Set a background color for the canvas to make it easier to tell where the canvas is for debugging purposes.
        canvas.style().set_property("background-color", "crimson");
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
            import { handleWasmCall } from "./wasm_custom_event.js";
            let button = document.createElement("button");
            button.innerHTML = "Favourite Number?";
            button.onclick = handleWasmCall;
            document.body.appendChild(button);
            "#,
        );

        let first_child = body.first_child();
        match first_child {
            Some(node) => body.insert_before(&script, Some(&node)).unwrap(),
            None => body.append_child(&script).unwrap(),
        };
    }
}
