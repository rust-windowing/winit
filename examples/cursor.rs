#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use std::path::Path;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    window::{CustomCursorIcon, NamedCursorIcon, WindowBuilder},
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new().build(&event_loop).unwrap();
    window.set_title("A fantastic window!");

    let mut cursor_idx = 0;
    let custom_cursor_icon = load_icon(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/icon.png"
    )));

    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    if cursor_idx < CURSORS.len() {
                        println!("Setting cursor to \"{:?}\"", CURSORS[cursor_idx]);
                        window.set_cursor_icon(CURSORS[cursor_idx]);
                        cursor_idx += 1;
                    } else {
                        println!("Setting cursor to custom");
                        window.set_cursor_icon(custom_cursor_icon.clone());
                        cursor_idx = 0;
                    }
                }
                WindowEvent::RedrawRequested => {
                    fill::fill_window(&window);
                }
                WindowEvent::CloseRequested => {
                    elwt.exit();
                }
                _ => (),
            }
        }
    })
}

const CURSORS: &[NamedCursorIcon] = &[
    NamedCursorIcon::Default,
    NamedCursorIcon::Crosshair,
    NamedCursorIcon::Pointer,
    NamedCursorIcon::Move,
    NamedCursorIcon::Text,
    NamedCursorIcon::Wait,
    NamedCursorIcon::Help,
    NamedCursorIcon::Progress,
    NamedCursorIcon::NotAllowed,
    NamedCursorIcon::ContextMenu,
    NamedCursorIcon::Cell,
    NamedCursorIcon::VerticalText,
    NamedCursorIcon::Alias,
    NamedCursorIcon::Copy,
    NamedCursorIcon::NoDrop,
    NamedCursorIcon::Grab,
    NamedCursorIcon::Grabbing,
    NamedCursorIcon::AllScroll,
    NamedCursorIcon::ZoomIn,
    NamedCursorIcon::ZoomOut,
    NamedCursorIcon::EResize,
    NamedCursorIcon::NResize,
    NamedCursorIcon::NeResize,
    NamedCursorIcon::NwResize,
    NamedCursorIcon::SResize,
    NamedCursorIcon::SeResize,
    NamedCursorIcon::SwResize,
    NamedCursorIcon::WResize,
    NamedCursorIcon::EwResize,
    NamedCursorIcon::NsResize,
    NamedCursorIcon::NeswResize,
    NamedCursorIcon::NwseResize,
    NamedCursorIcon::ColResize,
    NamedCursorIcon::RowResize,
];

fn load_icon(path: &Path) -> CustomCursorIcon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    CustomCursorIcon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
