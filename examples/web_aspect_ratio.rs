pub fn main() {
    println!("This example must be run with cargo run-wasm --example web_aspect_ratio")
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{HtmlCanvasElement, HtmlElement};
    use winit::{
        dpi::PhysicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::{Window, WindowBuilder},
    };

    const EXPLANATION: &str = "
This example draws a circle in the middle of a 4/1 aspect ratio canvas which acts as a useful demonstration of winit's resize handling on web.
Even when the browser window is resized or aspect-ratio of the div changed the circle should always:
* Fill the entire width or height of the canvas (whichever is smaller) without exceeding it.
* Be perfectly round
* Not be blurry or pixelated (there is no antialiasing so you may still see jagged edges depending on the DPI of your monitor)
";

    #[wasm_bindgen(start)]
    pub fn run() {
        console_log::init_with_level(log::Level::Debug).expect("error initializing logger");
        let event_loop = EventLoop::new();

        let window = WindowBuilder::new()
            .with_title("A fantastic window!")
            // A small default size is used to better demonstrate issues that come from failing to update the size
            .with_inner_size(PhysicalSize::new(100, 100))
            .build(&event_loop)
            .unwrap();

        let canvas = create_canvas(&window);

        // Render once with the size info we currently have
        render_circle(&canvas, window.inner_size());

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent {
                    event: WindowEvent::Resized(resize),
                    window_id,
                } if window_id == window.id() => {
                    render_circle(&canvas, resize);
                }
                _ => (),
            }
        });
    }

    pub fn create_canvas(window: &Window) -> HtmlCanvasElement {
        use winit::platform::web::WindowExtWebSys;

        let web_window = web_sys::window().unwrap();
        let document = web_window.document().unwrap();
        let body = document.body().unwrap();

        let parent_div = document.create_element("div").unwrap();
        parent_div
            .dyn_ref::<HtmlElement>()
            .unwrap()
            .style()
            .set_css_text("margin: auto; width: 50%; aspect-ratio: 4 / 1;");
        body.append_child(&parent_div).unwrap();

        // Set a background color for the canvas to make it easier to tell the where the canvas is for debugging purposes.
        let canvas = window.canvas();
        canvas
            .style()
            .set_css_text("display: block; width: 100%; height: 100%; background-color: crimson;");
        parent_div.append_child(&canvas).unwrap();

        let explanation = document.create_element("pre").unwrap();
        explanation.set_text_content(Some(EXPLANATION));
        body.append_child(&explanation).unwrap();

        canvas
    }

    pub fn render_circle(canvas: &HtmlCanvasElement, size: PhysicalSize<u32>) {
        log::info!("rendering circle with canvas size: {:?}", size);
        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();

        context.begin_path();
        context
            .arc(
                size.width as f64 / 2.0,
                size.height as f64 / 2.0,
                size.width.min(size.height) as f64 / 2.0,
                0.0,
                std::f64::consts::PI * 2.0,
            )
            .unwrap();
        context.fill();
    }
}
