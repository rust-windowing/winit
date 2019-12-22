use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

pub fn main() {
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    #[cfg(feature = "web-sys")]
    {
        use winit::platform::web::WindowExtWebSys;

        let canvas = window.canvas();

        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let body = document.body().unwrap();

        body.append_child(&canvas)
            .expect("Append canvas to HTML body");
    }

    #[cfg(feature = "stdweb")]
    {
        use std_web::web::INode;
        use winit::platform::web::WindowExtStdweb;

        let canvas = window.canvas();

        let document = std_web::web::document();
        let body: std_web::web::Node = document.body().expect("Get HTML body").into();

        body.append_child(&canvas);
    }

    event_loop.run(move |event, _, control_flow| {
        #[cfg(feature = "web-sys")]
        log::debug!("{:?}", event);

        #[cfg(feature = "stdweb")]
        std_web::console!(log, "%s", format!("{:?}", event));

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => *control_flow = ControlFlow::Wait,
        }
    });
}

#[cfg(feature = "web-sys")]
mod wasm {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn run() {
        console_log::init_with_level(log::Level::Debug);

        super::main();
    }
}
