#![allow(clippy::single_match)]

use std::path::Path;

use simple_logger::SimpleLogger;
use winit::{
    event::Event,
    event_loop::EventLoop,
    window::{Icon, WindowBuilder},
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();

    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("An iconic window!")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        if let Event::WindowEvent { event, .. } = event {
            use winit::event::WindowEvent::*;
            match event {
                CloseRequested => control_flow.set_exit(),
                DroppedFile(path) => {
                    window.set_cursor_icon(load_icon(&path));
                }
                _ => (),
            }
        } else if let Event::RedrawRequested(_) = event {
            fill::fill_window(&window);
        }
    })
}

fn load_icon(path: &Path) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
