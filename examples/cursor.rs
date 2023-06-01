#![allow(clippy::single_match)]

use std::path::Path;

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorIcon, Icon, WindowBuilder},
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new().build(&event_loop).unwrap();
    window.set_title("A fantastic window!");

    let mut cursor_idx = 0;

    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            state: ElementState::Pressed,
                            physical_key: PhysicalKey::Code(KeyCode::KeyC),
                            ..
                        },
                    ..
                } => {
                    println!("Setting cursor to custom");
                    let path: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");
                    let icon = load_icon(Path::new(path));
                    window.set_cursor_custom_icon(icon.clone());
                }
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    println!("Setting cursor to \"{:?}\"", CURSORS[cursor_idx]);
                    window.set_cursor_icon(CURSORS[cursor_idx]);
                    if cursor_idx < CURSORS.len() - 1 {
                        cursor_idx += 1;
                    } else {
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

const CURSORS: &[CursorIcon] = &[
    CursorIcon::Default,
    CursorIcon::Crosshair,
    CursorIcon::Pointer,
    CursorIcon::Move,
    CursorIcon::Text,
    CursorIcon::Wait,
    CursorIcon::Help,
    CursorIcon::Progress,
    CursorIcon::NotAllowed,
    CursorIcon::ContextMenu,
    CursorIcon::Cell,
    CursorIcon::VerticalText,
    CursorIcon::Alias,
    CursorIcon::Copy,
    CursorIcon::NoDrop,
    CursorIcon::Grab,
    CursorIcon::Grabbing,
    CursorIcon::AllScroll,
    CursorIcon::ZoomIn,
    CursorIcon::ZoomOut,
    CursorIcon::EResize,
    CursorIcon::NResize,
    CursorIcon::NeResize,
    CursorIcon::NwResize,
    CursorIcon::SResize,
    CursorIcon::SeResize,
    CursorIcon::SwResize,
    CursorIcon::WResize,
    CursorIcon::EwResize,
    CursorIcon::NsResize,
    CursorIcon::NeswResize,
    CursorIcon::NwseResize,
    CursorIcon::ColResize,
    CursorIcon::RowResize,
];

fn load_icon(path: &Path) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    // Assumes that hotspot is at the left top corner.
    Icon::from_rgba_cursor(icon_rgba, icon_width, icon_height, 0, 0).expect("Failed to open icon")
}
