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
        *control_flow = ControlFlow::Wait;

        #[cfg(feature = "web-sys")]
        {
            use wasm_bindgen::closure::Closure;
            use wasm_bindgen::JsCast;
            use web_sys::{FileReader, HtmlImageElement, Url};

            log::debug!("{:?}", event);
            match &event {
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::DroppedFile(file),
                } => {
                    // let reader = FileReader::new().unwrap();
                    // let f = Closure::wrap(Box::new(move |event: web_sys::Event| {
                    //     let reader: FileReader = event.target().unwrap().dyn_into().unwrap();
                    //     let file = reader.result().unwrap();
                    //     log::debug!("dropped filed {:?}", file);

                    //     let window = web_sys::window().unwrap();
                    //     let document = window.document().unwrap();
                    //     let body = document.body().unwrap();

                    //     let img = document.create_element("img").unwrap();
                    //     img.set_src(Url::create_object_url_with_blob(file));
                    //     img.set_height(60);
                    //     img.set_onload(|| {
                    //         Url::revoke_object_url(img.src());

                    //     });

                    //     body.append_child(&img)
                    //         .expect("Append canvas to HTML body");

                    // }) as Box<dyn FnMut(_)>);
                    // reader.set_onload(Some(f.as_ref().unchecked_ref()));
                    // reader.read_as_array_buffer(&file.slice().unwrap());
                    // f.forget();

                    let window = web_sys::window().unwrap();
                    let document = window.document().unwrap();
                    let body = document.body().unwrap();

                    let img = HtmlImageElement::new().unwrap();
                    img.set_src(&Url::create_object_url_with_blob(file).unwrap());
                    img.set_height(60);

                    let f = Closure::wrap(Box::new(|event: web_sys::Event| {
                        let img: HtmlImageElement = event.target().unwrap().dyn_into().unwrap();
                        Url::revoke_object_url(&img.src());
                    }) as Box<dyn FnMut(_)>);

                    img.set_onload(Some(f.as_ref().unchecked_ref()));

                    f.forget();

                    body.append_child(&img).expect("Append img to body");
                }

                _ => {}
            }
        }

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
            _ => (),
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
