//! Demonstrates capability to create in-app draggable regions for client-side decoration support.

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, KeyEvent, MouseButton, StartCause, WindowEvent},
    event_loop::EventLoop,
    keyboard::Key,
    window::{CursorIcon, ResizeDirection, WindowBuilder},
};

const BORDER: f64 = 8.0;

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_inner_size(winit::dpi::LogicalSize::new(600.0, 400.0))
        .with_min_inner_size(winit::dpi::LogicalSize::new(400.0, 200.0))
        .with_decorations(false)
        .build(&event_loop)
        .unwrap();

    let mut border = false;
    let mut cursor_location = None;

    event_loop.run(move |event, elwt| match event {
        Event::NewEvents(StartCause::Init) => {
            eprintln!("Press 'B' to toggle borderless")
        }
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => elwt.exit(),
            WindowEvent::CursorMoved { position, .. } => {
                if !window.is_decorated() {
                    let new_location =
                        cursor_resize_direction(window.inner_size(), position, BORDER);

                    if new_location != cursor_location {
                        cursor_location = new_location;
                        window.set_cursor_icon(cursor_direction_icon(cursor_location))
                    }
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(dir) = cursor_location {
                    let _res = window.drag_resize_window(dir);
                } else if !window.is_decorated() {
                    let _res = window.drag_window();
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Released,
                        logical_key: Key::Character(c),
                        ..
                    },
                ..
            } if matches!(c.as_ref(), "B" | "b") => {
                border = !border;
                window.set_decorations(border);
            }
            WindowEvent::RedrawRequested => {
                fill::fill_window(&window);
            }
            _ => (),
        },

        _ => (),
    })
}

fn cursor_direction_icon(resize_direction: Option<ResizeDirection>) -> CursorIcon {
    match resize_direction {
        Some(resize_direction) => match resize_direction {
            ResizeDirection::East => CursorIcon::EResize,
            ResizeDirection::North => CursorIcon::NResize,
            ResizeDirection::NorthEast => CursorIcon::NeResize,
            ResizeDirection::NorthWest => CursorIcon::NwResize,
            ResizeDirection::South => CursorIcon::SResize,
            ResizeDirection::SouthEast => CursorIcon::SeResize,
            ResizeDirection::SouthWest => CursorIcon::SwResize,
            ResizeDirection::West => CursorIcon::WResize,
        },
        None => CursorIcon::Default,
    }
}

fn cursor_resize_direction(
    win_size: winit::dpi::PhysicalSize<u32>,
    position: winit::dpi::PhysicalPosition<f64>,
    border_size: f64,
) -> Option<ResizeDirection> {
    enum XDirection {
        West,
        East,
        Default,
    }

    enum YDirection {
        North,
        South,
        Default,
    }

    let xdir = if position.x < border_size {
        XDirection::West
    } else if position.x > (win_size.width as f64 - border_size) {
        XDirection::East
    } else {
        XDirection::Default
    };

    let ydir = if position.y < border_size {
        YDirection::North
    } else if position.y > (win_size.height as f64 - border_size) {
        YDirection::South
    } else {
        YDirection::Default
    };

    Some(match xdir {
        XDirection::West => match ydir {
            YDirection::North => ResizeDirection::NorthWest,
            YDirection::South => ResizeDirection::SouthWest,
            YDirection::Default => ResizeDirection::West,
        },

        XDirection::East => match ydir {
            YDirection::North => ResizeDirection::NorthEast,
            YDirection::South => ResizeDirection::SouthEast,
            YDirection::Default => ResizeDirection::East,
        },

        XDirection::Default => match ydir {
            YDirection::North => ResizeDirection::North,
            YDirection::South => ResizeDirection::South,
            YDirection::Default => return None,
        },
    })
}
