#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    window::{NamedCursorIcon, WindowBuilder},
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
