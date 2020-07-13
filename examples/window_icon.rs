use std::{ffi::OsStr, fs::File, path::Path};
use winit::{
    dpi::PhysicalSize,
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    window::{Icon, RgbaIcon, WindowBuilder},
};

fn main() {
    simple_logger::init().unwrap();

    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/icons/icon_folder/"
    ));

    let icon = load_icon(&path);

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
    let decode_png = |path: &Path| {
        let decoder = png::Decoder::new(File::open(path).unwrap());
        let (info, mut reader) = decoder.read_info().unwrap();

        let mut rgba = vec![0; info.buffer_size()];
        reader.next_frame(&mut rgba).unwrap();

        (rgba, PhysicalSize::new(info.width, info.height))
    };
    if path.is_file() {
        if path.extension() == Some(OsStr::new("png")) {
            let (icon_rgba, icon_size) = decode_png(path);
            Icon::from_rgba(&icon_rgba, icon_size).unwrap()
        } else {
            panic!("unsupported file extension: {:?}", path.extension());
        }
    } else if path.is_dir() {
        let path = path.to_owned();
        Icon::from_rgba_fn(move |size, _| {
            let path = path.join(format!("{}.png", size.width));
            let (icon_rgba, icon_size) = decode_png(&path);
            Ok(RgbaIcon::from_rgba(icon_rgba, icon_size))
        })
    } else {
        panic!("path {} is neither file nor directory", path.display());
    }
}
