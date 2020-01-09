extern crate image;
use std::path::Path;
use winit::{
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    window::{Icon, WindowBuilder},
};

fn main() {
    simple_logger::init().unwrap();

    // You'll have to choose an icon size at your own discretion. On X11, the desired size varies
    // by WM, and on Windows, you still have to account for screen scaling. Here we use 32px,
    // since it seems to work well enough in most cases. Be careful about going too high, or
    // you'll be bitten by the low-quality downscaling built into the WM.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");

    let icon = load_icon(Path::new(path));

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("An iconic window!")
        // At present, this only does anything on Windows and X11, so if you want to save load
        // time, you can put icon loading behind a function that returns `None` on other platforms.
        .with_window_icon(Some(icon))
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { event, .. } = event {
            use winit::event::WindowEvent::*;
            match event {
                CloseRequested => *control_flow = ControlFlow::Exit,
                DroppedFile(path) => {
                    window.set_window_icon(Some(load_icon(&path)));
                }
                _ => (),
            }
        }
    });
}

fn load_icon(path: &Path) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path).expect("Failed to open icon path");
        use image::{GenericImageView, Pixel};
        let (width, height) = image.dimensions();
        let mut rgba = Vec::with_capacity((width * height) as usize * 4);
        for (_, _, pixel) in image.pixels() {
            rgba.extend_from_slice(&pixel.to_rgba().data);
        }
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
