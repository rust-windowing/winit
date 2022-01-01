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

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowExtWebSys;

        let canvas = window.canvas();

        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let parent_div = document.get_element_by_id("app").unwrap();

        parent_div
            .append_child(&canvas)
            .expect("Append canvas to HTML body");
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        #[cfg(target_arch = "wasm32")]
        {
            log::debug!("{:?}", event);

            // Getting access to browser logs requires a lot of setup on mobile devices.
            // So we implement this basic logging system into the page to give developers an easy alternative.
            // As a bonus its also kind of handy on desktop.
            if let Event::WindowEvent { event, .. } = &event {
                let window = web_sys::window().unwrap();
                let document = window.document().unwrap();
                let list = document.get_element_by_id("event_log").unwrap();
                let log = document.create_element("li").unwrap();
                log.set_text_content(Some(&format!("{:?}", event)));
                list.insert_before(&log, list.first_child().as_ref())
                    .unwrap();
            }
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => (),
        }
    });
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn run() {
        console_log::init_with_level(log::Level::Debug).expect("error initializing logger");

        super::main();
    }
}
