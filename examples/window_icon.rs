use std::{fs::File, path::Path};
use winit::{
    dpi::PhysicalSize,
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

    let icon = load_icon(Path::new(&path));

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
    let (icon_rgba, icon_size) = {
        let decoder = png::Decoder::new(File::open(path).expect("Failed to open icon path"));
        let (info, mut reader) = decoder.read_info().expect("Failed to decode icon PNG");

        let mut rgba = vec![0; info.buffer_size()];
        reader.next_frame(&mut rgba).unwrap();

        (rgba, PhysicalSize::new(info.width, info.height))
    };
    Icon::from_rgba(&icon_rgba, icon_size).expect("Failed to open icon")
}
